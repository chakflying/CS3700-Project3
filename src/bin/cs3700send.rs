use std::io::{self, Read};
use clap::{App, Arg};
#[macro_use]
extern crate log;

fn main() {
    env_logger::init();
    debug!("Started");
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer);
    info!("{}",buffer);
}