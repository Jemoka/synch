pub mod sync;
pub use sync::*;

pub mod rtc;
pub use rtc::*;

use anyhow::Result;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // build an api and config for the connection
    let api = rtc::get_api()?;
    let config = rtc::get_config_from_stun_servers(&["stun:stun.l.google.com:19302"]);
    let peer_connection = Arc::new(api.new_peer_connection(config).await?);

    // create connection helper 
    let mut cnx = Connection::new(peer_connection.clone(), None);

    // create the channel to talk over
    cnx.channel("test").await?;

    // generate an offer for our peer
    let offer = cnx.offer().await?;

    // <meanwhile, on a different machine>
    let answer = cnx.answer(&offer).await?;
    // </meanwhile, on a different machine>

    // accept our peer's answer
    cnx.accept(&answer).await?;

    // wait forever
    let _ = tokio::signal::ctrl_c().await;

    // and destroy 
    cnx.close().await?;

    Ok(())
}
