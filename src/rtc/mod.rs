//! Real Time Communication

/// maximum size of message which fits onto a MTU
pub const MAX_MSG_SIZE_BYTES: usize = 1500;
/// default STUN servers to use
pub const DEFAULT_STUN_SERVERS:&[&str] = &[
    "stun:stun.cloudflare.com:3478",
    "stun:stun.l.google.com:19302",
];
/// default size of a queue before we block
pub const DEFAULT_QUEUE_SIZE: usize = 16;

mod utils;
mod connection;
mod agent;

pub use utils::*;
pub use connection::*;
pub use agent::*;

