use bytes::Bytes;
use std::pin::Pin;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::future::Future;
use std::collections::HashMap;
use tokio::sync::mpsc::{Sender, Receiver, channel};
use base64::prelude::{BASE64_URL_SAFE, Engine as _};
use webrtc::{data_channel::RTCDataChannel,
             data::data_channel::DataChannel,
             peer_connection::{peer_connection_state::RTCPeerConnectionState,
                               sdp::session_description::RTCSessionDescription,
                               RTCPeerConnection}};

use crate::MAX_MSG_SIZE_BYTES;

#[derive(Debug)]
pub enum ConnectionType {
    HEAD,
    CHILD
}

type QueueTuple = (String, Vec<u8>);
type Queue = (Sender<QueueTuple>, Receiver<QueueTuple>);

pub struct Connection {
    cnx: Arc<RTCPeerConnection>,
    cnx_type: Option<ConnectionType>,

    read_queue: Arc<Queue>,
    write_queues: Arc<Mutex<HashMap<String, Sender<QueueTuple>>>>,
    queue_size: usize
}

impl Connection {
    pub fn new(connection: Arc<RTCPeerConnection>,
               queue_size: Option<usize>) -> Connection {

        let qs = match queue_size { Some(x) => x, None => 1};

        Connection {
            cnx: connection,
            cnx_type: None,
            read_queue: Arc::new(channel(qs)),
            write_queues: Arc::new(Mutex::new(HashMap::new())),
            queue_size: qs
        }
    }

    pub async fn close(&self) -> Result<()> {
        self.cnx.close().await?;

        Ok(())
    }

    pub async fn channel(&self, name: &str) -> Result<Arc<RTCDataChannel>> {
        let channel = self.cnx.create_data_channel(name, None).await?;
        let read_queue = self.read_queue.clone();
        let write_queues = self.write_queues.clone();
        let capacity = self.queue_size;

        let _ = Connection::register_channel(channel.clone(), read_queue,
                                             write_queues, capacity);

        Ok(channel)
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
                        read_queue: Arc<Queue>,
                        write_queues: Arc<Mutex<HashMap<String, Sender<QueueTuple>>>>,
                        capacity:usize) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>
    {
        println!("New Data Channel: {}", d.label());
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

            channel.clone().on_open(Box::new(move || {
                Box::pin(async move {
                    let raw = match channel.detach().await {
                        Ok(raw) => raw,
                        Err(err) => {
                            println!("data channel {} detach got err: {err}",
                                     channel.label());
                            return;
                        }
                    };

                    let rc = raw.clone();
                    tokio::spawn(async move {
                        Connection::_read_worker(rc,
                                                 read_queue.0.clone(),
                                                 channel.label().to_owned()).await;
                    });

                    tokio::spawn(async move {
                        Connection::_write_worker(raw, reciever).await;
                    });
                })
            }));
        })
    }


    async fn listen(&self, desc: RTCSessionDescription) -> Result<String> {
        // keep a pointer to the queue
        let read_queue = self.read_queue.clone();
        let write_queues = self.write_queues.clone();
        let capacity = self.queue_size;

        // create a handler for new data channels
        self.cnx.on_data_channel(Box::new(move |d: Arc<RTCDataChannel>| {
            // copy the queue pointers to share with the registration function 
            let read_queue = read_queue.clone();
            let write_queues = write_queues.clone();

            Connection::register_channel(d, read_queue, write_queues, capacity)
        }));

        // wait for ICE gather; TODO this disables trickle ICE, which should
        // be implemented eventually
        let mut gather_complete = self.cnx.gathering_complete_promise().await;
        self.cnx.set_local_description(desc).await?;
        let _ = gather_complete.recv().await;

        // TODO do something about this instead of just printing it
        self.cnx.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
            println!("Peer Connection State has changed: {s}");

            if s == RTCPeerConnectionState::Failed {
                println!("Peer Connection has gone to failed exiting");
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

