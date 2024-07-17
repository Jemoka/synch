pub mod sync;
pub use sync::*;

pub mod rtc;
pub use rtc::*;

use anyhow::Result;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // build an api and config
    let api = rtc::get_api()?;
    let config = rtc::get_config_from_stun_servers(&["stun:stun.l.google.com:19302"]);
    let peer_connection = Arc::new(api.new_peer_connection(config).await?);

    // create connection helper 
    let mut cnx = Connection::new(peer_connection.clone(), None);
    let offer = cnx.offer().await?;

    println!("offer = {}", &offer);

    cnx.channel("test").await?;

    let _ = tokio::signal::ctrl_c().await;

    cnx.close().await?;

    // cnx.answer(&offer).await?;

    // // read in a response
    // let mut response = String::new();
    // std::io::stdin().read_line(&mut response)?;
    // response = response.trim_end().to_owned();

    // dbg!(response);

    Ok(())
}
