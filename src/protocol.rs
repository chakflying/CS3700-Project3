use chrono::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::net::UdpSocket;
use std::num::Wrapping;
use std::time::{Duration, Instant};
use integer_encoding::*;

/// Bind to a random port to open a UDP socket. Returns the socket.
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

/// Specify port when binding to UDP socket, useful for development.
pub fn bind_socket_with_port(ip: &str, port: &str) -> UdpSocket {
    let socket_bind;
    socket_bind = UdpSocket::bind(format!("{}:{}", ip, port));
    eprintln!("{:?} [bound] {}", Local::now(), port);
    socket_bind.unwrap()
}

// Reference from rust-quic: https://github.com/flier/rust-quic/tree/develop
pub struct State {
    pub connected: bool,
    pub initial_sent_packet_num: u32,
    pub initial_received_packet_num: u32,
    pub time_of_last_received_packet: Option<Instant>,
    pub time_of_last_sent_new_packet: Option<Instant>,
    pub connection_creation_time: Option<Instant>,
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
    pub struct PacketType: u8 {
        const INIT = 0b0001;
        const NORM = 0b0010;
    }
}
impl_serde_for_bitflags!(PacketType);

bitflags! {
    pub struct IntLength: u8 {
        const U8 = 0b0001;
        const U16 = 0b0010;
        const U32 = 0b0100;
        const U64 = 0b1000;
    }
}
impl_serde_for_bitflags!(IntLength);

#[derive(PartialEq, Clone, Debug)]
pub struct Header {
    pub packet_type: PacketType,
    pub packet_num: u32,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct Frame {
    pub frame_type: FrameType,
    #[serde(with = "serde_bytes")]
    pub frame_data: Vec<u8>,
}

/// Build a header by taking in the PacketType and the packet number used. Returns header bytes.
pub fn build_header(packet_type: PacketType, packet_num: u32) -> Vec<u8> {
    let mut output = vec![0 as u8; 1];
    output.extend(bincode::serialize(&packet_type).unwrap());
    output.extend(packet_num.encode_var_vec());
    output[0] = output.len() as u8 - 1;
    debug!("Encoded header size: {}", output.len() as u8);
    output
}

/// Decode a vector of bytes into a Header struct. Returns the header and bytes consumed.
pub fn decode_header(data: &Vec<u8>) -> (Header,usize) {
    let header_size = data[0] as usize;
    debug!("Decoded header size: {}", header_size + 1);
    let packet_type: PacketType = bincode::deserialize(&data[1..2]).unwrap();
    let packet_num = u32::decode_var(&data[2..1+header_size]).0;
    (Header {
        packet_type,
        packet_num,
    }, header_size +1)
}

/// Build a data frame by taking in the data. Returns frame bytes.
pub fn build_frame_data(data: &Vec<u8>) -> Vec<u8> {
    let mut output = Vec::<u8>::new();
    let frame = Frame {
        frame_type: FrameType::DATA,
        frame_data: data.clone(),
    };
    let encoded_frame = bincode::serialize(&frame).unwrap();
    unsafe { output.extend(std::mem::transmute::<u16, [u8; 2]>(encoded_frame.len() as u16).iter()); }
    output.extend(encoded_frame);
    debug!("Encoded frame size: {}", output.len());
    output
}

/// Decode a vector of bytes into a Frame. Returns the frame and bytes consumed.
pub fn decode_frame(data: &Vec<u8>) -> (Frame,usize) {
    let frame_size = u16::from_ne_bytes([data[0], data[1]]) as usize;
    debug!("Decoded frame size: {}", frame_size + 2);
    (bincode::deserialize(&data[2..2+frame_size]).unwrap(), frame_size + 2)
}
