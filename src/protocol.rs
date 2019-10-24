use chrono::prelude::*;
use rand::Rng;
use serde::ser::{Serialize, Serializer, SerializeStruct};
use std::net::UdpSocket;
use std::time::{Duration, Instant};

pub fn bind_socket(ip: &str) -> UdpSocket {
    let mut rng = rand::thread_rng();
    let mut port = rng.gen_range(29170, 29998);
    let mut socket_bind;
    while {
        socket_bind = UdpSocket::bind(format!("{}:{}", ip, port));
        socket_bind.is_err()
    } {
        port = rng.gen_range(29170, 29998);
    }
    eprintln!("{:?} [bound] {}", Local::now(), port);
    socket_bind.unwrap()
}

// Reference from rust-quic: https://github.com/flier/rust-quic/tree/develop
pub struct State {
    pub connected: bool,
    pub time_of_last_received_packet: Instant,
    pub time_of_last_sent_new_packet: Instant,
    pub connection_creation_time: Instant,
}

bitflags! {
    pub struct FrameType: u8 {
        const PING = 0b0001;
        const ACK = 0b0010;
        const CLOSE = 0b0100;
        const DATA = 0b1000;
    }
}
impl_serde_for_bitflags!(FrameType);

bitflags! {
    pub struct IntLength: u8 {
        const U8 = 0b0001;
        const U16 = 0b0010;
        const U32 = 0b0100;
        const U64 = 0b1000;
    }
}
impl_serde_for_bitflags!(IntLength);

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Header {
    packet_num_length: IntLength,
    packet_num: u64,
}

impl Serialize for Header {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Header", 2)?;
        state.serialize_field("Packet number Length", &self.packet_num_length)?;
        state.serialize_field("Packet number", &self.packet_num)?;
        state.end()
    }
}