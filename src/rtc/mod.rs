//! Real Time Communication

/// maximum size of message which fits onto a MTU
pub const MAX_MSG_SIZE_BYTES: usize = 1500;

mod utils;
mod connection;

pub use utils::*;
pub use connection::*;

