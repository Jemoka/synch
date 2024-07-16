use anyhow::Result;
use std::sync::Arc;
use webrtc::peer_connection::{sdp::session_description::RTCSessionDescription, RTCPeerConnection};
use base64::prelude::{BASE64_URL_SAFE, Engine as _};

#[derive(Debug)]
pub enum ConnectionType {
    HEAD,
    CHILD
}

#[derive(Debug)]
pub struct Connection {
    cnx: Arc<RTCPeerConnection>,
    cnx_type: Option<ConnectionType>
}

impl Connection {
    pub fn new(connection: Arc<RTCPeerConnection>) -> Connection {
        Connection {
            cnx: connection,
            cnx_type: None
        }
    }

    async fn listen(&self, desc: RTCSessionDescription) -> Result<String> {
        let mut gather_complete = self.cnx.gathering_complete_promise().await;
        self.cnx.set_local_description(desc).await?;
        let _ = gather_complete.recv().await;

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

    /// make this connection a [ConnectionType::CHILD] node, creating an answer to an offer
    ///
    /// # Arguments
    ///
    /// * `offer` - Base64 encoded offer string.
    pub async fn answer(&mut self, offer: &str) -> Result<String> {
        self.cnx_type = Some(ConnectionType::CHILD);

        // decode session
        let deserialized:RTCSessionDescription = serde_json::from_str(
            &String::from_utf8(BASE64_URL_SAFE.decode(offer)?)?
        )?;
        self.cnx.set_remote_description(deserialized).await?;

        // generate an answer and listen for it
        self.listen(self.cnx.create_answer(None).await?).await
    }
}

