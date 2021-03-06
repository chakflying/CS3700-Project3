use std::io;
use std::io::prelude::*;
use std::collections::{HashMap, VecDeque};
use std::cmp;
use chrono::prelude::*;
use rand::Rng;
extern crate clap;
use clap::{Arg, App};
use std::time::{Duration, Instant};

extern crate pretty_env_logger;
#[macro_use]
extern crate log;

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
        .arg(
            Arg::with_name("random bytes")
            .short("r")
            .long("random")
            .required(false)
            .takes_value(true)
            .help("Generate random bytes as input"),
        )
        .get_matches();
    let client = args.value_of("client").unwrap();
    let random = args.value_of("random bytes").unwrap_or("none");
    let client_ip = &client[0..client.find(":").expect("Argument Incorrect formatting")];
    let client_port = &client[client.find(":").unwrap() + 1..];


    let socket = protocol::bind_socket(client_ip);
    socket.connect(format!("127.0.0.1:{}", client_port)).expect("connect to receiver failed");
    debug!("Receiver connected");

    let initial_packet_num = rng.gen_range(1, u8::max_value() as u64);
    let mut state = protocol::State {
        connected: true,
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
        last_PTO: 0,
        last_PTO_time: None,
        max_RTT: 0,
        min_RTT: 0,
        latest_RTT: 0,
        smoothed_RTT: 0,
        RTT_variance: 0,
        congestion_window: 14720,
        max_congestion_window: 14720,
        bytes_in_flight: 0,
        slow_start_threshold: usize::max_value(),
        congestion_recovery_start_time: None,
        packet_sent: 0,
        packet_lost: 0,

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

    let mut buffer = Vec::new();
    if random != "none" {
        let length:u32 = random.parse().unwrap();
        buffer = (0..length).map(|_| { rand::random::<u8>() }).collect();
    } else {
        io::stdin().read_to_end(&mut buffer).expect("Error on reading input");
    }

    state.build_new_data_packet(&buffer);
    state.send_a_packet_in_queue();
    while state.established == false {
        state.receive_packet();
//        if !received && state.should_send_ACK() { state.send_ACK(); }
        state.detect_packet_lost();
        state.resend_lost_packet_data(&buffer);
    }

    while state.closing == None {
        if state.bytes_in_flight <= state.congestion_window {
            state.resend_lost_packet_data(&buffer);
        }
        if state.bytes_in_flight <= 3000 {
            state.send_all_in_queue();
            state.send_new_data(&buffer);
        }
        let received = state.receive_packet();
        if !received && state.should_send_ACK() { state.send_ACK(); }
        state.detect_packet_lost();
        // if state.sent_end_byte_processed && state.lost_packets.len() == 0 && state.send_state.send_queue.len() == 0 && state.bytes_in_flight == 0 { more_to_send = false; }
    }
    eprintln!("{:?} [completed]", Local::now());

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