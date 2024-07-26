use bytes::Bytes;
use std::pin::Pin;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
use std::future::Future;
use std::collections::HashMap;
use tokio::sync::mpsc::{Sender, Receiver};
use base64::prelude::{BASE64_URL_SAFE, Engine as _};
use webrtc::{data_channel::RTCDataChannel,
             data::data_channel::DataChannel,
             peer_connection::{peer_connection_state::RTCPeerConnectionState,
                               sdp::session_description::RTCSessionDescription,
                               RTCPeerConnection}};
use log::{error, debug};

use crate::MAX_MSG_SIZE_BYTES;

#[derive(Debug)]
pub enum ConnectionType {
    HEAD,
    CHILD
}

type QueueTuple = (String, Vec<u8>);

pub struct Connection {
    cnx: Arc<RTCPeerConnection>,
    cnx_type: Option<ConnectionType>,

    new_channel_notify: Arc<Notify>,

    // the first mutex is for insertions to the map; the second mutex
    // is for the reciever itself, blocking each data channel queue 
    read_queues: Arc<Mutex<HashMap<String, Arc<Mutex<Receiver<QueueTuple>>>>>>,
    write_queues: Arc<Mutex<HashMap<String, Sender<QueueTuple>>>>,
    queue_size: usize
}

impl Connection {
    pub fn new(connection: Arc<RTCPeerConnection>,
               queue_size: Option<usize>) -> Connection {

        let qs = match queue_size { Some(x) => x, None => super::DEFAULT_QUEUE_SIZE};

        Connection {
            cnx: connection,
            cnx_type: None,
            new_channel_notify: Arc::new(Notify::new()),
            read_queues: Arc::new(Mutex::new(HashMap::new())),
            write_queues: Arc::new(Mutex::new(HashMap::new())),
            queue_size: qs
        }
    }

    /// read from a channel, if exists and is non empty
    ///
    /// # Notes
    /// blocks until something is given or queue is dead
    /// returns [Option::None] if nothing is there.
    /// blocks until this channel you want exists
    pub async fn recv(&self, channel: &str) -> Option<QueueTuple> {
        let queuetex: Arc<Mutex<Receiver<QueueTuple>>> = {
            loop {
                // lock the global mutex briefly to get the correct
                // channel, checking if it exists (if it doesn't,
                // wait for our Semiphore)
                let queuetable = self.read_queues.lock();
                if let Some(n) = queuetable.await.get(channel) {
                    break n.clone();
                }

                self.new_channel_notify.notified().await;
            }
        };

        // now. lock the second lock until we got something; this
        // means we will lock every other member trying to read
        // from this channel (which shouldn't be more than 1 thread
        // anyway.)
        let mut queue = queuetex.lock().await;
        queue.recv().await
    }

    /// write to a channel, if exists and has capacity
    ///
    /// # Notes
    /// blocks until this channel you want exists
    pub async fn send(&self, channel: &str, data: Vec<u8>) -> Result<()> {
        let queue: Sender<QueueTuple> = {
            loop {
                // lock the global mutex briefly to get the correct
                // channel, checking if it exists (if it doesn't,
                // wait for our Semiphore)
                let queuetable = self.write_queues.lock();
                if let Some(n) = queuetable.await.get(channel) {
                    break n.clone();
                }

                self.new_channel_notify.notified().await;
            }
        };

        // whoosh
        queue.send((channel.into(), data)).await?;

        Ok(())
    }

    pub async fn close(&self) -> Result<()> {
        self.cnx.close().await?;

        Ok(())
    }

    pub async fn channel(&self, name: &str) -> Result<()> {
        let channel = self.cnx.create_data_channel(name, None).await?;
        let read_queues = self.read_queues.clone();
        let write_queues = self.write_queues.clone();
        let new_channel = self.new_channel_notify.clone();
        let capacity = self.queue_size;

        let _ = Connection::register_channel(channel.clone(), new_channel, read_queues,
                                             write_queues, capacity).await;

        Ok(())
    }

    async fn _read_worker(d: Arc<DataChannel>, queue: Sender<QueueTuple>, name: String) {
        let mut buffer = vec![0u8; MAX_MSG_SIZE_BYTES];

        loop {
            let n = match d.read(&mut buffer).await {
                // number of bytes read
                Ok(n) => n,
                // data channel exited
                Err(_) => {
                    return;
                }
            };

            // push to our queue
            let _ = queue.send((name.clone(), buffer[..n].to_vec())).await;
        }
    }

    async fn _write_worker(d: Arc<DataChannel>, mut queue: Receiver<QueueTuple>) {
        loop {
            let (_, data) = match queue.recv().await {
                // number of bytes read
                Some(n) => n,
                // data channel exited
                None => {
                    return;
                }
            };

            // push to rtc; if error, our channel closed
            if let Err(_) = d.write(&Bytes::from(data)).await {
                return;
            }
        }
    }

    fn register_channel(d: Arc<RTCDataChannel>,
                        notify: Arc<Notify>,
                        read_queues: Arc<Mutex<HashMap<String, Arc<Mutex<Receiver<QueueTuple>>>>>>,
                        write_queues: Arc<Mutex<HashMap<String, Sender<QueueTuple>>>>,
                        capacity:usize) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>
    {
        debug!("data channel connected: name '{}'", d.label());
        let channel = d.clone();

        Box::pin(async move {
            // create a new channel to write to this channel
            // and write the send end down for others' use
            let reciever = {
                let mut guarded_write_hmp = write_queues.lock().await;
                let (snd, recv) = tokio::sync::mpsc::channel(capacity);
                guarded_write_hmp.insert(
                    channel.label().to_owned(),
                    snd
                );
                recv
            };

            let sender = {
                let mut guarded_write_hmp = read_queues.lock().await;
                let (snd, recv) = tokio::sync::mpsc::channel(capacity);
                guarded_write_hmp.insert(
                    channel.label().to_owned(),
                    Arc::new(Mutex::new(recv))
                );
                snd
            };

            channel.clone().on_open(Box::new(move || {
                Box::pin(async move {
                    let raw = match channel.detach().await {
                        Ok(raw) => raw,
                        Err(err) => {
                            error!("data channel detach got err: name '{}', error '{err}'",
                                   channel.label());
                            return;
                        }
                    };

                    let rc = raw.clone();
                    tokio::spawn(async move {
                        Connection::_read_worker(rc, sender,
                                                 channel.label().to_owned()).await;
                    });

                    tokio::spawn(async move {
                        Connection::_write_worker(raw, reciever).await;
                    });

                    notify.notify_waiters();
                })
            }));
        })
    }


    async fn listen(&self, desc: RTCSessionDescription) -> Result<String> {
        // keep a pointer to the queue
        let read_queue = self.read_queues.clone();
        let write_queues = self.write_queues.clone();
        let new_channel = self.new_channel_notify.clone();
        let capacity = self.queue_size;

        // create a handler for new data channels
        self.cnx.on_data_channel(Box::new(move |d: Arc<RTCDataChannel>| {
            // copy the queue pointers to share with the registration function 
            let read_queue = read_queue.clone();
            let write_queues = write_queues.clone();
            let new_channel = new_channel.clone();

            Connection::register_channel(d, new_channel, read_queue, write_queues, capacity)
        }));

        // wait for ICE gather; TODO this disables trickle ICE, which should
        // be implemented eventually
        let mut gather_complete = self.cnx.gathering_complete_promise().await;
        self.cnx.set_local_description(desc).await?;
        let _ = gather_complete.recv().await;

        // TODO do something about this instead of just printing it
        self.cnx.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
            debug!("peer connection state changed to: {s}");

            if s == RTCPeerConnectionState::Failed {
                error!("peer connection state failed!");
            }

            Box::pin(async {})
        }));

        match self.cnx.local_description().await {
            Some(local_desc) => Ok(BASE64_URL_SAFE.encode(serde_json::to_string(&local_desc)?)),
            None => panic!("failed to register local description")
        }
    }

    /// make this connection a [ConnectionType::HEAD] node, creating an offer to respond to
    pub async fn offer(&mut self) -> Result<String> {
        self.cnx_type = Some(ConnectionType::HEAD);

        // generate an offer and listen for it
        self.listen(self.cnx.create_offer(None).await?).await
    }

    /// accept an `answer` generated by the remote, responding to [offer]
    ///
    /// # Arguments
    ///
    /// * `answer` - Base64 encoded answer string.
    pub async fn accept(&mut self, answer: &str) -> Result<()> {
        // decode session
        let deserialized:RTCSessionDescription = serde_json::from_str(
            &String::from_utf8(BASE64_URL_SAFE.decode(answer)?)?
        )?;
        self.cnx.set_remote_description(deserialized).await?;

        Ok(())
    }

    /// make this connection a [ConnectionType::CHILD] node, creating an answer to an offer
    ///
    /// # Arguments
    ///
    /// * `offer` - Base64 encoded offer string.
    ///
    /// # Returns
    /// Our answer to the `offer`.
    pub async fn answer(&mut self, offer: &str) -> Result<String> {
        self.cnx_type = Some(ConnectionType::CHILD);
        self.accept(offer).await?;

        // generate an answer and listen for it
        self.listen(self.cnx.create_answer(None).await?).await
    }
}

