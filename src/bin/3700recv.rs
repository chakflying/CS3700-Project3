use chrono::prelude::*;
use clap::{App, Arg};
use rand::Rng;
use std::collections::{HashMap, VecDeque};
use std::io::{self, Read, Write};
use std::{str, cmp};
use std::time::{Duration, Instant};
extern crate crypto;
use crypto::md5::Md5;
use crypto::digest::Digest;

extern crate pretty_env_logger;
#[macro_use]
extern crate log;

use PROJECT3::protocol;

fn main() {
    pretty_env_logger::init();
    debug!("Receiver Started");
    // let socket = protocol::bind_socket("127.0.0.1");
    let socket = protocol::bind_socket_with_port("127.0.0.1", "28899");

    let mut rng = rand::thread_rng();
    let initial_packet_num = rng.gen_range(1, u8::max_value() as u64);
    let mut state = protocol::State {
        connected: false,
        closing: None,
        established: false,
        initial_sent_packet_num: initial_packet_num,
        last_packet_num: initial_packet_num - 1,
        initial_received_packet_num: 0,
        time_of_last_received_packet: None,
        time_of_last_sent_new_packet: None,
        connection_creation_time: Some(Instant::now()),
        sent_largest_ACKed: 0,
        sent_largest_lost: 0,
        sent_packets: HashMap::new(),
        sent_ack_largest: HashMap::new(),
        sent_end_byte_processed: false,

        received_largest: 0,
        received_packets: HashMap::new(),
        time_of_last_packet_reorder: None,
        next_byte_offset: 0,
        lost_packets: VecDeque::new(),
        ack_starting_packet_num: 0,

        socket: socket,

        PTO_amount: 0,
        max_RTT: 0,
        min_RTT: 0,
        latest_RTT: 0,
        smoothed_RTT: 0,
        RTT_variance: 0,
        congestion_window: 14720,
        bytes_in_flight: 0,
        slow_start_threshold: usize::max_value(),
        congestion_recovery_start_time: None,

        send_state: protocol::StreamSendState {
            sent_data: HashMap::new(),
            send_queue: VecDeque::new(),
        },
        receive_state: protocol::StreamReceiveState {
            received_data: HashMap::new(),
            assembled_data: Vec::new(),
            end_received: None,
        },
    };

    while state.established == false {
        let received = state.receive_packet();
        if !received && state.should_send_ACK() { state.send_ACK(); }
        state.detect_packet_lost();
    }

    let mut more_to_receive = true;
    while more_to_receive {
        let mut received = false;
        while {
            if state.receive_packet() && !received { received = true; }
            state.receive_packet()
         } {}
        if !received && state.should_send_ACK() { state.send_ACK(); }
        state.detect_packet_lost();
        if !received && state.receive_state.end_received != None && state.assemble_remaining_data() { more_to_receive = false; }
    }
    eprintln!("{:?} [completed]", Local::now());
    print!("{}", str::from_utf8(&state.receive_state.assembled_data).unwrap());

    let mut hasher = Md5::new();
    hasher.input(&state.receive_state.assembled_data);
    info!("Hash of received data: {}", hasher.result_str());

    let mut timer = Instant::now();
    let mut close_attempt = 0;
    while state.connected && close_attempt < 3 {
        while {state.receive_packet()} {}
        if timer.elapsed() > cmp::max(2 * Duration::from_nanos(state.smoothed_RTT), Duration::from_millis(100)) {
            state.send_close_packet();
            close_attempt += 1;
            timer = Instant::now();
        }
    }
}
