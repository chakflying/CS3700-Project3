use std::io::{Read, Write};
use clap::{App, Arg};
#[macro_use]
extern crate log;

fn main() {
    env_logger::init();
    debug!("Started");
}