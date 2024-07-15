pub mod sync;
pub use sync::*;

pub mod rtc;

// use ciborium::{into_writer, from_reader};

#[tokio::main]
async fn main() {
    let _api = rtc::get_api();
}
