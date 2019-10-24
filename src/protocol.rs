use std::time::{Duration,Instant};

// Reference from rust-quic: https://github.com/flier/rust-quic/tree/develop
pub struct State {
    pub connected: bool,
    pub time_of_last_received_packet: Instant,
    pub time_of_last_sent_new_packet: Instant,
    pub connection_creation_time: Instant,
}