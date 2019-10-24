use std::io::{self, Read};
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
    let mut rng = rand::thread_rng();
    let mut port = rng.gen_range(29170, 29998);
    let mut socket_bind;
    while {
        socket_bind = UdpSocket::bind(format!("{}:{}",client_ip,port));
        socket_bind.is_err()
    } { port = rng.gen_range(29170, 29998); }
    let socket = socket_bind.unwrap();
    eprintln!("{:?} [bound] {}", Local::now(), port);
    socket.connect(format!("127.0.0.1:{}",client_port)).expect("connect to receiver failed");
    let mut buf = [0; 10];
    socket.send(&buf).expect("Send Failed");
}