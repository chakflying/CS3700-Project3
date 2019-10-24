use std::io::{self, Read};
use std::time::{Duration,Instant};
use chrono::prelude::*;
use clap::{App, Arg};
use std::net::UdpSocket;
use rand::Rng;
extern crate pretty_env_logger;
#[macro_use] extern crate log;

use PROJECT3::protocol;

fn main() {
    pretty_env_logger::init();
    debug!("Receiver Started");
    let mut rng = rand::thread_rng();
    let mut port = rng.gen_range(29170, 29998);
    let mut socket_bind;
    while {
        socket_bind = UdpSocket::bind(format!("127.0.0.1:{}",port));
        socket_bind.is_err()
    } { port = rng.gen_range(29170, 29998); }
    let socket = socket_bind.unwrap();
    eprintln!("{:?} [bound] {}", Local::now(), port);
     let mut buf = [0; 2000];
    let (amt, src) = socket.recv_from(&mut buf).unwrap();
    debug!("{} bytes received.", amt);
}