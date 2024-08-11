pub mod sync;
pub use sync::prelude::*;

use anyhow::Result;

// use log::info;

#[tokio::main]
async fn main() -> Result<()> {
    // let mut amy:SyncedList<u8> = SyncedList::new();
    // let mut bob:SyncedList<u8> = amy.clone();
    // amy.push(5);
    // amy.push(7);
    // *amy.get(0).unwrap() = 12;
    // bob.push(12);
    // bob.push(18);

    // bob.replay(amy.tape());
    // amy.replay(bob.tape());
    // assert_eq!(bob.tape().len(), 0);

    // *bob.get(0).unwrap() = 17;
    // *amy.get(0).unwrap() = 11;

    // bob.replay(amy.tape());
    // amy.replay(bob.tape());

    // dbg!(amy);
    // dbg!(bob);

    Ok(())
}
