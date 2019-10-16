use std::io::{self, Read};
use clap::{App, Arg};
#[macro_use]
extern crate log;

fn main() {
    env_logger::init();
    debug!("Receiver Started");
}