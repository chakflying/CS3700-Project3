use std::io;
use std::io::prelude::*;
use std::time::{Duration,Instant};
use chrono::prelude::*;
use std::net::UdpSocket;
use rand::Rng;
use clap::{App, Arg};
extern crate pretty_env_logger;
#[macro_use] extern crate log;

use PROJECT3::protocol;

fn main() {
    pretty_env_logger::init();
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

    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let buffer = stdin.fill_buf().unwrap();

    socket.send(&buffer).expect("Send Failed");
    let length = buffer.len();
    stdin.consume(length);
}