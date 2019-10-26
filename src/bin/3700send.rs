use std::io;
use std::io::prelude::*;
use std::time::{Duration,Instant};
use std::cmp;
use chrono::prelude::*;
use std::net::UdpSocket;
use rand::Rng;
use clap::{App, Arg};
extern crate pretty_env_logger;
#[macro_use] extern crate log;

use PROJECT3::protocol;

fn main() {
    pretty_env_logger::init();
    let mut rng = rand::thread_rng();
    debug!("Sender Started");
    let args = App::new("CS3700 Project 3")
        .author("Nelson Chan <chan.chak@husky.neu.edu>")
        .arg(
            Arg::with_name("client")
                .index(1)
                .required(true)
                .help("The receiver to connect to"),
        )
        .get_matches();
    let client = args.value_of("client").unwrap();
    let client_ip = &client[0..client.find(":").expect("Argument Incorrect formatting")];
    let client_port = &client[client.find(":").unwrap()+1..];
    let socket = protocol::bind_socket(client_ip);
    socket.connect(format!("127.0.0.1:{}",client_port)).expect("connect to receiver failed");
    debug!("Receiver connected");

    let state = protocol::State {
        connected: true,
        initial_sent_packet_num: rng.gen_range(1, u8::max_value() as u32),
        initial_received_packet_num: 0,
        time_of_last_received_packet: None,
        time_of_last_sent_new_packet: None,
        connection_creation_time: Some(Instant::now()),
    };

    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let buffer = stdin.fill_buf().unwrap();

    let mut packet = protocol::build_header(protocol::PacketType::INIT, state.initial_sent_packet_num);
    let data_frame = protocol::build_frame_data(&buffer[..cmp::min(1472-packet.len()-2-9,buffer.len())].into());
    packet.extend(data_frame);
    debug!("Packet of size {} constructed.", packet.len());

    socket.send(&packet).expect("Send Failed");
    let length = buffer.len();
    stdin.consume(length);
}