pub mod sync;
pub use sync::*;

pub mod rtc;
pub use rtc::*;

use anyhow::Result;

use std::fs;
// use std::io::{self, Read};

use log::info;
use env_logger::Env;

#[tokio::main]
async fn main() -> Result<()> {
    // initialize our logger
    let env = Env::default()
        .filter_or("RUST_LOG", "synch=debug");
    env_logger::init_from_env(env);

    // parent code
    let mut parent_agent = Agent::head()?;
    let mut offer = parent_agent.offer().await?;
    info!("parent offer: {}", offer.get());
    let line = fs::read_to_string("./tmp").unwrap();
    offer.answer(&line.trim()).await?;
    parent_agent.accept(offer)?;


    // child code
    let line = fs::read_to_string("./tmp").unwrap();
    let (answer, _child_agent) = Agent::child(&line.trim()).await?;
    info!("child answer: {}", answer);
        
    // let api = rtc::get_api()?;
    // let config = rtc::get_config_from_stun_servers(&[]);
    // let peer_connection = Arc::new(api.new_peer_connection(config).await?);

    // // create connection helper 
    // let mut cnx = Connection::new(peer_connection.clone(), None);

    // // create the channel to talk over
    // cnx.channel("test").await?;

    // // generate an offer for our peer
    // let offer = cnx.offer().await?;
    // info!("offer: {}", offer);

    // read a byte and block
    // let _ = io::stdin().read(&mut [0u8]).unwrap();
    // read the sync file
    // info!("done reading answer");

    // // <meanwhile, on a different machine>
    // // string for the line
    // // let answer = cnx.answer(&line.trim()).await?;
    // // println!("answer: {}", answer);
    // // </meanwhile, on a different machine>

    // // accept our peer's answer
    // cnx.accept(&line.trim()).await?;

    // // send some stuff
    // // <meanwhile, on a different machine>
    // // cnx.send("test", vec![3,4,5,6]).await?;
    // // </meanwhile, on a different machine>

    // // read some stuff
    // dbg!(cnx.recv("test").await.unwrap().1);

    // // wait forever
    // let _ = tokio::signal::ctrl_c().await;

    // // and destroy 
    // cnx.close().await?;

    Ok(())
}
