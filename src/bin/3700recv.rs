use chrono::prelude::*;
use clap::{App, Arg};
use rand::Rng;
use std::io::{self, Read};
use std::net::UdpSocket;
use std::time::{Duration, Instant};
extern crate pretty_env_logger;
#[macro_use]
extern crate log;

use PROJECT3::protocol;

fn main() {
    pretty_env_logger::init();
    debug!("Receiver Started");
    let socket = protocol::bind_socket("127.0.0.1");
    let mut buf = [0; 2000];
    while {
        let (amt, src) = socket.recv_from(&mut buf).unwrap();
        debug!("{} bytes received.", amt);
        println!("{:?}", &buf[..amt]);
        amt != 0
    } {}
}
