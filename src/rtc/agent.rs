use std::sync::{Arc, Mutex};
use anyhow::{Result, anyhow};
use webrtc::{api::API,
             peer_connection::configuration::RTCConfiguration};
use tokio::sync::mpsc::{Sender, Receiver, channel};
use futures::future::join_all;

use super::utils::*;
use super::DEFAULT_STUN_SERVERS;
use super::connection::Connection;

/// temporary Offer connection holder
///
/// # Notes
/// Why is this a seperate struct? we need to keep the
/// connection alive and eventually merge it back
///
/// # Examples
///
/// ```
/// let mut agent = Agent::head()?;
/// let mut offer = agent.offer().await?;
/// tell_peer(offer.get());
/// offer.accept(get_from_peer());
/// agent.accept(offer);
/// ```
pub struct Offer {
    cnx: Connection,
    offer: String,
    validated: bool
}

impl Offer {
    /// get the offer string
    pub fn get(&self) -> String {
        self.offer.clone()
    }
    /// answer the offer, validating it
    pub async fn answer(&mut self, answer: &str) -> Result<()> {
        self.validated = true;
        self.cnx.accept(answer).await?;
        Ok(())
    }
}

pub struct Channel {
    name: String,
    sender: Sender<Vec<u8>>,
    reciever: Receiver<Vec<u8>>,
}

/// agent for handling RTC connections and propegating messages
pub struct Agent {
    parent: Option<Connection>,
    children: Vec<Connection>,
    api_instance: API,
    config: RTCConfiguration,
    workers: Vec<tokio::task::JoinHandle<()>>,
    channels: Vec<Channel>
}

impl Agent {

    // to implement signaling: purpose---"get a parent"
    // pub fn signal(signaling server)
    //     which has .on_child() which calls self.child()

    /// synchronize a channel between parent and child
    pub async fn sync(&mut self, channel_name: &str) -> Result<()> {
        // create the channel in each of the children
        join_all(self.children
                 .iter()
                 .map(|x| x.channel(channel_name))).await;

        // create channels to and from the sender
        // "sender" is the end to send stuff to publish to network
        // "reciever" is the end to recieve stuff that the network published
        let (sender, publication_reciever) = channel(super::DEFAULT_QUEUE_SIZE);
        let (publication_sender, reciever) = channel(super::DEFAULT_QUEUE_SIZE);

        let shared = Channel {
            name: channel_name.to_owned(),
            sender: sender,
            reciever: reciever
        };

        // bubble child events up
        self.workers.push(
            tokio::spawn(async {
                loop {
                }
            })
        );

        Ok(())
    }

    /// create a head node
    pub fn head() -> Result<Agent> {
        Agent::configure_manually(None, DEFAULT_STUN_SERVERS)
    }

    /// create a child by accepting a new offer
    ///
    /// # Return
    /// A response to the parent offer and an Agent
    /// corresponding to the child.
    pub async fn child(offer: &str) -> Result<(String, Agent)> {
        let mut child = Agent::configure_manually(None, DEFAULT_STUN_SERVERS)?;
        let mut parent_cnx = child.create_connection().await?;

        let answer = parent_cnx.answer(offer).await?;
        child.parent = Some(parent_cnx);

        Ok((answer, child))
    }

    /// offer a new connection to a possible child
    pub async fn offer(&self) -> Result<Offer> {
        let mut child_cnx = self.create_connection().await?;
        let offer = child_cnx.offer().await?;

        Ok(Offer {
            cnx: child_cnx,
            offer: offer,
            validated: false
        })
    }

    /// accept a child connection
    pub fn accept(&mut self, validated_offer: Offer) -> Result<()> {
        if !validated_offer.validated {
            return Err(anyhow!("offer given to accept has not been answered yet!"));
        }

        self.children.push(validated_offer.cnx);

        Ok(())
    }

    pub fn configure_manually(parent: Option<Connection>, stun_servers: &[&str]) -> Result<Agent> {
        let api = get_api()?;
        let config = get_config_from_stun_servers(stun_servers);

        Ok(Agent {
            parent: parent,
            children: vec![],
            api_instance: api,
            config: config,
            workers: vec![],
            channels: vec![]
        })
    }

    async fn create_connection(&self) -> Result<Connection> {
        Ok(Connection::new(
            Arc::new(
                self.api_instance
                    .new_peer_connection(self.config.clone())
                    .await?
            ), None
        ))
    }
}


