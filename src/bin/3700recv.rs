use std::io::{self, Read};
use std::net::UdpSocket;
use std::time::{Duration, Instant};
use std::str;
use chrono::prelude::*;
use clap::{App, Arg};
use rand::Rng;
extern crate pretty_env_logger;
#[macro_use]
extern crate log;

use PROJECT3::protocol;

fn main() {
    pretty_env_logger::init();
    debug!("Receiver Started");
    // let socket = protocol::bind_socket("127.0.0.1");
    let socket = protocol::bind_socket_with_port("127.0.0.1", "28899");

    let state = protocol::State {
        connected: true,
        initial_sent_packet_num: 0,
        initial_received_packet_num: 0,
        time_of_last_received_packet: None,
        time_of_last_sent_new_packet: None,
        connection_creation_time: Some(Instant::now()),
    };

    let mut buf = [0; 2000];
    while {
        let (amt, src) = socket.recv_from(&mut buf).unwrap();
        debug!("{} bytes received.", amt);
        let (received_header, header_size) = protocol::decode_header(&buf[..amt].into());
        debug!("Received_header: {:?}", received_header);
        let (received_frame, frame_size) = protocol::decode_frame(&buf[header_size..amt].into());
        debug!("Received_frame: {:?}", received_frame);
        println!("{}", str::from_utf8(&received_frame.frame_data).unwrap());
        amt != 0
    } {}
}
