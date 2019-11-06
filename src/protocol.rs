#![allow(non_snake_case)]

use chrono::prelude::*;
use integer_encoding::*;
use rand::Rng;
use std::{cmp, io, str};
use std::net::UdpSocket;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Bind to a random port to open a UDP socket. Returns the socket.
pub fn bind_socket(ip: &str) -> UdpSocket {
    let mut rng = rand::thread_rng();
    let mut port = rng.gen_range(29170, 29998);
    let mut socket_bind;
    while {
        socket_bind = UdpSocket::bind(format!("{}:{}", ip, port));
        socket_bind.is_err()
    } {
        port = rng.gen_range(29170, 29998);
    }
    eprintln!("{:?} [bound] {}", Local::now(), port);
    let socket = socket_bind.unwrap();
    socket.set_nonblocking(true).unwrap();
    socket
}

/// Specify port when binding to UDP socket, useful for development.
pub fn bind_socket_with_port(ip: &str, port: &str) -> UdpSocket {
    let socket_bind;
    socket_bind = UdpSocket::bind(format!("{}:{}", ip, port));
    eprintln!("{:?} [bound] {}", Local::now(), port);
    let socket = socket_bind.unwrap();
    socket.set_nonblocking(true).unwrap();
    socket
}

// Reference from rust-quic: https://github.com/flier/rust-quic/tree/develop
#[derive(Debug)]
pub struct State {
    pub connected: bool,
    pub closing: Option<u64>,
    pub established: bool,
    pub initial_sent_packet_num: u64,
    pub last_packet_num: u64,
    pub time_of_last_sent_new_packet: Option<Instant>,
    pub connection_creation_time: Option<Instant>,
    pub sent_largest_ACKed: u64,
    pub sent_largest_lost: u64,
    pub sent_packets: HashMap<u64, SentPacket>,
    pub lost_packets: VecDeque<u64>,
    pub sent_ack_largest: HashMap<u64, u64>,
    pub sent_end_byte_processed: bool,

    pub initial_received_packet_num: u64,
    pub received_largest: u64,
    pub time_of_last_received_packet: Option<Instant>,
    pub received_packets: HashMap<u64, ReceivedPacket>,
    pub time_of_last_packet_reorder: Option<Instant>,
    pub ack_starting_packet_num: u64,

    pub next_byte_offset: usize,

    pub socket: UdpSocket,

    pub PTO_amount: u32,
    pub last_PTO: u64,
    pub last_PTO_time: Option<Instant>,
    pub max_RTT: u64,
    pub min_RTT: u64,
    pub latest_RTT: u64,
    pub smoothed_RTT: u64,
    pub RTT_variance: u64,
    pub congestion_window: usize,
    pub max_congestion_window: usize,
    pub last_max_congestion_window: usize,
    pub bytes_in_flight: usize,
    pub slow_start_threshold: usize,
    pub congestion_recovery_start_time: Option<Instant>,
    pub last_congestion_event: Option<Instant>,

    pub send_state: StreamSendState,
    pub receive_state: StreamReceiveState,
}

#[derive(PartialEq, Clone, Debug)]
pub struct StreamSendState {
    // pub byte_offset_NEXT: u64,
    pub sent_data: HashMap<u64, DataSegment>,
    pub send_queue: VecDeque<Packet>,
}

#[derive(PartialEq, Clone, Debug)]
pub struct StreamReceiveState {
    pub received_data: HashMap<u64, Vec<u8>>,
    pub assembled_data: Vec<u8>,
    pub end_received: Option<u64>,
}

#[derive(PartialEq, Clone, Debug)]
pub struct DataSegment {
    pub byte_offset: u64,
    pub length: usize,
}

#[derive(PartialEq, Clone, Debug)]
pub struct SentPacket {
    pub packet_num: u64,
    pub size: usize,
    pub time_sent: Instant,
    pub in_flight: bool,
    pub is_ack_only: bool,
}

#[derive(PartialEq, Clone, Debug)]
pub struct ReceivedPacket {
    pub packet_num: u64,
    pub time_received: Instant,
    pub ack_sent: bool,
    pub is_ack_only: bool,
}

bitflags! {
    pub struct FrameType: u8 {
        const PING = 0b0001;
        const ACK = 0b0010;
        const CLOSE = 0b0100;
        const DATA = 0b1000;
    }
}
impl_serde_for_bitflags!(FrameType);

bitflags! {
    pub struct PacketType: u8 {
        const INIT = 0b0001;
        const NORM = 0b0010;
    }
}
impl_serde_for_bitflags!(PacketType);

bitflags! {
    pub struct IntLength: u8 {
        const U8 = 0b0001;
        const U16 = 0b0010;
        const U32 = 0b0100;
        const U64 = 0b1000;
    }
}
impl_serde_for_bitflags!(IntLength);

#[derive(PartialEq, Clone, Debug)]
pub struct Header {
    pub packet_type: PacketType,
    pub packet_num: u64,
}

impl Header {
    pub fn serialize(&self) -> Vec<u8> {
        let mut output = vec![0 as u8; 1];
        output.extend(bincode::serialize(&self.packet_type).unwrap());
        output.extend(self.packet_num.encode_var_vec());
        output[0] = output.len() as u8 - 1;
//        debug!("Encoded header size: {}", output.len() as u8);
        output
    }
    pub fn deserialize(data: &Vec<u8>) -> (Header, usize) {
        let header_size = data[0] as usize;
        // debug!("Decoded header size: {}", header_size + 1);
        let packet_type: PacketType = bincode::deserialize(&data[1..2]).unwrap();
        let packet_num = u64::decode_var(&data[2..1 + header_size]).0;
        (
            Header {
                packet_type,
                packet_num,
            },
            header_size + 1,
        )
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct Frame {
    pub frame_type: FrameType,
    pub frame_data: Vec<u8>,
}

impl Frame {
    pub fn serialize(&self) -> Vec<u8> {
        let mut output = Vec::<u8>::new();
        // let encoded_frame = bincode::serialize(&self).unwrap();
        unsafe {
            output.extend(std::mem::transmute::<u16, [u8; 2]>((self.frame_data.len() + 1) as u16).iter());
        }
        output.extend(bincode::serialize(&self.frame_type).unwrap());
        output.extend(&self.frame_data);
//        debug!("Encoded frame size: {}", output.len());
        output
    }
    pub fn deserialize(data: &Vec<u8>) -> (Frame, usize) {
        let frame_size = u16::from_ne_bytes([data[0], data[1]]) as usize;
        // debug!("Decoded frame size: {}", frame_size + 2);
        (
            Frame {
                frame_type: bincode::deserialize(&data[2..3]).unwrap(),
                frame_data: data[3..2 + frame_size].into(),
            },
            frame_size + 2,
        )
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct DataFrame {
    pub end: bool,
    pub byte_offset: u64,
    pub data: Vec<u8>,
}

impl DataFrame {
    pub fn serialize(&self) -> Vec<u8> {
        let mut output = vec![0 as u8; 1];
        if self.end == true {
            output[0] = 1;
        }
        output.extend(self.byte_offset.encode_var_vec());
        output.extend(self.data.clone());
//        debug!("Encoded dataframe size: {}", output.len());
        output
    }
    pub fn deserialize(input: &Vec<u8>) -> DataFrame {
        let end = input[0] == 1;
        let byte_offset_decode = u64::decode_var(&input[1..]);
        let data = &input[byte_offset_decode.1 + 1..];
        DataFrame {
            end: end,
            byte_offset: byte_offset_decode.0,
            data: data.to_vec(),
        }
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct AckFrame {
    pub largest_ack: u64,
    pub ack_delay: u64,
    pub ack_ranges: Vec<u32>,
}

impl AckFrame {
    pub fn is_acked(&self, packet_num: u64) -> bool {
        if packet_num == self.largest_ack { return true; }
        let mut acks = Vec::<bool>::new();
        let mut flip = true;
        for range in self.ack_ranges.iter() {
            acks.extend(c![flip, for x in 0..*range]);
            flip = !flip;
        }
        for (i, acked) in acks.iter().enumerate() {
            if packet_num == self.largest_ack - (i + 1) as u64 { return *acked; }
        }
        false
    }
    pub fn serialize(&self) -> Vec<u8> {
        let mut output = Vec::<u8>::new();
        output.extend(self.largest_ack.encode_var_vec());
        output.extend(self.ack_delay.encode_var_vec());
        for range in self.ack_ranges.iter() {
            output.extend(range.encode_var_vec());
        }
        output
    }
    pub fn deserialize(input: &Vec<u8>) -> AckFrame {
        let mut current_offset = 0;
        let (largest_ack, offset) = u64::decode_var(&input[..]);
        current_offset += offset;
        let (ack_delay, offset) = u64::decode_var(&input[current_offset..]);
        current_offset += offset;
        let mut ack_ranges = Vec::<u32>::new();
        while current_offset < input.len() {
            let (range, offset) = u32::decode_var(&input[current_offset..]);
            ack_ranges.push(range);
            current_offset += offset;
        }
        AckFrame {
            largest_ack,
            ack_delay,
            ack_ranges,
        }
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct Packet {
    pub header: Header,
    pub frames: Vec<Frame>,
}

impl Packet {
    pub fn len(&self) -> usize {
        let mut output = Vec::<u8>::new();
        output.extend(self.header.serialize());
        for frame in self.frames.iter() {
            output.extend(frame.serialize());
        }
        output.len()
    }
    pub fn serialize(&self) -> Vec<u8> {
        let mut output = Vec::<u8>::new();
        output.extend(self.header.serialize());
        for frame in self.frames.iter() {
            output.extend(frame.serialize());
        }
        output
    }
    pub fn deserialize(input: &Vec<u8>) -> Packet {
        let (header, header_size) = Header::deserialize(&input);
        let mut frames = Vec::<Frame>::new();
        let mut current_offset = header_size;
        while {
            let (frame, frame_size) = Frame::deserialize(&input[current_offset..].into());
            frames.push(frame);
            current_offset += frame_size;
            current_offset < input.len()
        } {}
        Packet {
            header,
            frames,
        }
    }
    pub fn is_ack_only(&self) -> bool {
        if self.frames.len() == 1 && self.frames[0].frame_type == FrameType::ACK {
            return true;
        } else {
            return false;
        }
    }
}

impl State {
    pub fn receive_packet(&mut self) -> bool {
        let mut buf = [0; 2000];
        let num_bytes_read;

        let result = match self.socket.recv_from(&mut buf) {
            Ok(n) => Some(n),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => None,
            Err(e) => {
                error!("IO error occurred: {}", e);
                std::process::exit(0);
            }
        };
        if result != None {
            num_bytes_read = result.unwrap().0;
        } else {
            return false;
        }
        if !self.connected {
            self.socket.connect(result.unwrap().1).expect("connect to sender failed");
            self.connected = true;
        }
        let packet = Packet::deserialize(&buf[..num_bytes_read].into());
        debug!("Received packet size {}: {{packet_type: {:?}, packet_num: {}  Frame_type: {:?}}}", num_bytes_read, packet.header.packet_type, packet.header.packet_num, c![frame.frame_type, for frame in packet.frames.iter()]);
        let packet_num = packet.header.packet_num;
        if self.received_packets.contains_key(&packet_num) {
            return false;
        }
        if self.received_largest + 1 != packet_num {
            self.time_of_last_packet_reorder = Some(Instant::now());
        }
        if self.received_largest < packet_num { self.received_largest = packet_num; }
        self.received_packets.insert(packet_num, ReceivedPacket { packet_num, time_received: Instant::now(), ack_sent: false, is_ack_only: packet.is_ack_only() });
        self.time_of_last_received_packet = Some(Instant::now());
        if packet.header.packet_type == PacketType::INIT {
            self.initial_received_packet_num = packet_num;
            self.ack_starting_packet_num = packet_num;
        }
        let should_send_ack = self.should_send_ACK();
        for frame in packet.frames.iter() {
            if frame.frame_type == FrameType::DATA {
                let dataframe = DataFrame::deserialize(&frame.frame_data);
                eprintln!("{:?} [recv data] {} ({}) {}", Local::now(), dataframe.byte_offset,dataframe.data.len(), if packet_num == self.received_largest {"ACCEPTED (in-order)"} else {"ACCEPTED (out-of-order)"});
                self.on_data_received(&dataframe);
            } else if frame.frame_type == FrameType::ACK {
                let ackframe = AckFrame::deserialize(&frame.frame_data);
                self.on_ack_received(&ackframe);
            } else if frame.frame_type == FrameType::CLOSE {
                if self.closing != None { self.connected = false; return true; }
                let mut packet = self.build_new_ack_packet();
                packet.frames.push(self.generate_close_frame());
                self.closing = Some(packet.header.packet_num);
                self.send_packet(packet);
                return true;
            }
        }
        if should_send_ack {
            self.send_ACK();
        }
        return true;
    }
    pub fn should_send_ACK(&mut self) -> bool {
        let mut ack_skipped = false;
        if self.received_largest == 0 || self.connected == false { return false; }
        if self.time_of_last_packet_reorder != None && (self.time_of_last_packet_reorder.unwrap().elapsed().as_nanos() as u64) < (1 / 8 * self.smoothed_RTT) { debug!("Sending ACK because of packet reorder."); return true; }
        for packet_num in c![x, for x in self.ack_starting_packet_num..self.received_largest+1] {
            match self.received_packets.get(&packet_num) {
                None => {}
                Some(received) => {
                    if received.is_ack_only && (received.time_received.elapsed().as_nanos() as u64 <= self.smoothed_RTT) { return false; }
                    if received.ack_sent == false && self.established == false { debug!("Sending ACK because not established."); return true; }
                    if received.ack_sent == false && received.time_received.elapsed() > Duration::from_millis(5) {
                        debug!("Sending ACK because max_ack_delay reached."); 
                        return true;
                    } else if received.ack_sent == false {
                        if ack_skipped { debug!("Sending ACK because 2 unacked packets."); return true; } else {
                            ack_skipped = true;
                        }
                    }
                }
            }
        }
        return false;
    }
    pub fn send_ACK(&mut self) {
        let packet = self.build_new_ack_packet();
        self.send_packet(packet);
    }
    pub fn send_PTO(&mut self) {
        let mut packet = self.build_new_ack_packet();
        packet.frames.push(Frame {frame_type: FrameType::PING, frame_data: vec![0]});
        self.send_packet(packet);
    }
    pub fn send_all_in_queue(&mut self) {
        while self.bytes_in_flight < self.congestion_window && self.send_state.send_queue.len() != 0 {
            if self.send_a_packet_in_queue() == false {break;}
        }
    }
    pub fn send_new_data(&mut self, data: &Vec<u8>) {
        // if self.send_state.send_queue.len() != 0 { info!("Send queue not empty when calling send_new_data()"); return; }
        while self.bytes_in_flight < self.congestion_window - 1472 && !self.sent_end_byte_processed {
            self.build_new_data_packet(data);
            if self.send_a_packet_in_queue() == false {break;}
        }
    }
    pub fn build_new_empty_packet(&mut self) -> Packet {
        let header = Header {
            packet_type: if self.established == true { PacketType::NORM } else { PacketType::INIT },
            packet_num: self.last_packet_num + 1,
        };
        self.last_packet_num += 1;
        Packet {
            header,
            frames: vec![],
        }
    }
    pub fn build_new_data_packet(&mut self, data: &Vec<u8>) -> DataSegment {
        let offset = self.next_byte_offset;
        let header = Header {
            packet_type: if self.established == true { PacketType::NORM } else { PacketType::INIT },
            packet_num: self.last_packet_num + 1,
        };
        let avaliable_bytes = 1472 - header.serialize().len() - 4 - (offset as u64).encode_var_vec().len();
        let end = offset + avaliable_bytes >= data.len();
        if end { self.sent_end_byte_processed = true; }
        let data_end = cmp::min(data.len(), offset + avaliable_bytes);
        let dataframe = DataFrame {
            end,
            byte_offset: offset as u64,
            data: data[offset..data_end].to_vec(),
        };
        let frame = Frame {
            frame_type: FrameType::DATA,
            frame_data: dataframe.serialize(),
        };
        self.last_packet_num += 1;
        self.next_byte_offset += data_end - offset;
        let data_segment = DataSegment { byte_offset: offset as u64, length: data_end - offset };
        debug!("Constructing packet from new segment: {:?}", data_segment);
        self.send_state.sent_data.insert(header.packet_num, DataSegment { byte_offset: offset as u64, length: data_end - offset });
        self.send_state.send_queue.push_back(Packet { header, frames: vec![frame] });
        data_segment
    }
    pub fn build_new_data_packet_from_segment(&mut self, data: &Vec<u8>, data_segment: DataSegment) -> DataSegment {
        let offset = data_segment.byte_offset as usize;
        let header = Header {
            packet_type: if self.established == true { PacketType::NORM } else { PacketType::INIT },
            packet_num: self.last_packet_num + 1,
        };
        let avaliable_bytes = 1472 - header.serialize().len() - 4 - (offset as u64).encode_var_vec().len();
        if avaliable_bytes < data_segment.length { 
            info!("Packet does not have enough space to send this data segment");
            let new_segment = DataSegment {
                byte_offset: data_segment.byte_offset + avaliable_bytes as u64,
                length: data_segment.length - avaliable_bytes,
            };
            self.build_new_data_packet_from_segment(data, new_segment);
        }
        let end = offset + data_segment.length >= data.len();
        let data_end = cmp::min(cmp::min(data.len(), offset + data_segment.length), offset + avaliable_bytes);
        let dataframe = DataFrame {
            end,
            byte_offset: offset as u64,
            data: data[offset..data_end].to_vec(),
        };
        let frame = Frame {
            frame_type: FrameType::DATA,
            frame_data: dataframe.serialize(),
        };
        let data_segment = DataSegment { byte_offset: offset as u64, length: data_end - offset };
        debug!("Constructing packet from lost segment: {:?}", data_segment);
        self.last_packet_num += 1;
        self.send_state.sent_data.insert(header.packet_num, DataSegment { byte_offset: offset as u64, length: data_end - offset });
        self.send_state.send_queue.push_front(Packet { header, frames: vec![frame] });
        data_segment
    }
    pub fn build_new_ack_packet(&mut self) -> Packet {
        let header = Header {
            packet_type: if self.established == true { PacketType::NORM } else { PacketType::INIT },
            packet_num: self.last_packet_num + 1,
        };
        let ackframe = self.generate_ackframe();
        self.sent_ack_largest.insert(header.packet_num, ackframe.largest_ack);
        let frame = Frame {
            frame_type: FrameType::ACK,
            frame_data: ackframe.serialize(),
        };
        self.last_packet_num += 1;
        Packet {
            header,
            frames: vec![frame],
        }
    }
    pub fn send_a_packet_in_queue(&mut self) -> bool {
//        debug!("Checking the queue to send packet: {} packets", self.send_state.send_queue.len());
        if self.send_state.send_queue.len() == 0 { return false; }
        let packet = self.send_state.send_queue.pop_front().unwrap();
        if self.bytes_in_flight + packet.len() > self.congestion_window {
            self.send_state.send_queue.push_front(packet);
            // debug!("Queue is full, not sending any more.");
            return false;
        }
        self.send_packet(packet);
        return true;
    }
    pub fn generate_ackframe(&mut self) -> AckFrame {
        self.received_packets.get_mut(&self.received_largest).unwrap().ack_sent = true;
        let mut ack_ranges = Vec::new();
        let mut current_num = self.received_largest - 1;
        let mut current_counter = 1;
        let mut flip = true;
        debug!("Generating ACK frame, largest packet: {}", self.ack_starting_packet_num);
        while current_num >= self.ack_starting_packet_num {
            let result = self.received_packets.contains_key(&current_num);
            if result == true {
                self.received_packets.get_mut(&current_num).unwrap().ack_sent = true;
            }
            if result == flip {
                current_counter += 1;
            } else {
                ack_ranges.push(current_counter);
                current_counter = 1;
                flip = !flip;
            }
            current_num -= 1;
        }
        ack_ranges.push(current_counter);
        AckFrame {
            largest_ack: self.received_largest,
            ack_delay: self.received_packets.get(&self.received_largest).unwrap().time_received.elapsed().as_nanos() as u64,
            ack_ranges,
        }
    }
    pub fn send_packet(&mut self, packet: Packet) {
        if !self.connected { return; }
        let sent_packet = SentPacket {
            packet_num: packet.header.packet_num,
            size: packet.len(),
            time_sent: Instant::now(),
            in_flight: true,
            is_ack_only: packet.is_ack_only(),
        };
        self.sent_packets.insert(packet.header.packet_num, sent_packet.clone()); 
        self.on_packet_sent(sent_packet);
        let packet_bytes = packet.serialize();
        debug!("Sending packet of size {}.", packet_bytes.len());
        self.socket.send(&packet_bytes).expect("Send Failed");
    }
    pub fn on_packet_sent(&mut self, sent_packet: SentPacket) {
        self.time_of_last_sent_new_packet = Some(Instant::now());
        self.cc_on_packet_sent(sent_packet.size);
    }
    pub fn on_data_received(&mut self, data_frame: &DataFrame) {
        debug!("Processing DataFrame: {{ end:{}, offset:{} }}", data_frame.end, data_frame.byte_offset);
        if !self.receive_state.received_data.contains_key(&data_frame.byte_offset) {
            self.receive_state.received_data.insert(data_frame.byte_offset, data_frame.data.clone());
        }
        // debug!("Data: {}", str::from_utf8(&data_frame.data).unwrap());
        if data_frame.end {
            self.receive_state.end_received = Some(data_frame.byte_offset + data_frame.data.len() as u64);
        }
    }
    pub fn on_ack_received(&mut self, ack_frame: &AckFrame) {
        debug!("Processing AckFrame: {:?}", ack_frame);
        self.PTO_amount = 0;
        if self.sent_largest_ACKed == 0 {
            self.sent_largest_ACKed = ack_frame.largest_ack;
        } else {
            self.sent_largest_ACKed = cmp::max(ack_frame.largest_ack, self.sent_largest_ACKed);
        }
        if self.closing != None && self.closing.unwrap() < self.sent_largest_ACKed { self.connected = false; return; }
        let new_latest_ack = self.sent_packets.contains_key(&ack_frame.largest_ack);
        if new_latest_ack {
            self.latest_RTT = (Instant::now() - self.sent_packets.get(&ack_frame.largest_ack).unwrap().time_sent).as_nanos() as u64;
            self.update_RTT(ack_frame.ack_delay);
            if self.established == false { self.established = true; }
        }
        let new_acked_packets = self.get_new_acked_packets(ack_frame);
        if new_acked_packets.len() == 0 { return; }
        for acked_packet in new_acked_packets.iter() {
            self.cc_on_packet_acked(acked_packet);
        }
        self.detect_packet_lost();
    }
    pub fn congestion_event(&mut self, sent_time: Instant) {
        if !self.cc_is_in_congestion_recovery(sent_time) {
            debug!("Congestion event started.");
            self.last_congestion_event = Some(Instant::now());
            self.congestion_recovery_start_time = Some(Instant::now());
            self.max_congestion_window = self.congestion_window;
            self.set_max_congestion_windows();
            self.congestion_window = (self.congestion_window as f64 * ( 1.0 + (( self.get_cubic_increase() - self.congestion_window as f64 ) / self.congestion_window as f64))) as usize;
            self.congestion_window = cmp::max(self.congestion_window, 14720);
            self.slow_start_threshold = self.congestion_window;
            debug!("Congestion window reduced to {}. Bytes in flight: {}", self.congestion_window, self.bytes_in_flight);
        }
    }
    pub fn get_lost_timeout(&self) -> u64 {
        let mut output = cmp::max(self.smoothed_RTT, self.latest_RTT);
        output = cmp::max(output, Duration::from_millis(2).as_nanos() as u64);
        (output as f64 * 9.0 / 8.0 * (1.0 + self.RTT_variance as f64 / self.smoothed_RTT as f64)) as u64
    }
    pub fn get_PTO(&self) -> u64 {
        let mut output;
        if self.smoothed_RTT == 0 {
            output = Duration::from_millis(100).as_nanos() as u64;
        } else {
            output = self.smoothed_RTT + cmp::max(4 * self.RTT_variance, Duration::from_millis(1).as_nanos() as u64) + Duration::from_millis(1).as_nanos() as u64;
        }
        if self.PTO_amount > 0 {
            output = self.last_PTO * 2;
        }
        output
    }
    pub fn get_new_acked_packets(&mut self, ack_frame: &AckFrame) -> Vec<SentPacket> {
        let mut acked_packets = Vec::<SentPacket>::new();
        let mut acks = Vec::<bool>::new();
        let mut flip = true;
        for range in ack_frame.ack_ranges.iter() {
            acks.extend(c![flip, for _x in 0..*range]);
            flip = !flip;
        }
        for (i, acked) in acks.iter().enumerate() {
            if *acked {
                let this_packet_num = ack_frame.largest_ack - (i as u64);
//                debug!("Packet {} is consider ACKed.", this_packet_num);
                match self.sent_packets.remove(&this_packet_num) {
                    Some(sent_packet) => acked_packets.push(sent_packet),
                    None => {}
                };
            }
        }
        acked_packets
    }
    pub fn assemble_remaining_data(&mut self) -> bool {
        while self.receive_state.received_data.contains_key(&(self.receive_state.assembled_data.len() as u64)) {
            self.receive_state.assembled_data.extend(self.receive_state.received_data.get(&(self.receive_state.assembled_data.len() as u64)).unwrap().clone());
        }
        if self.receive_state.end_received != None && self.receive_state.end_received.unwrap() == self.receive_state.assembled_data.len() as u64 {
            debug!("Data reported as complete.");
            return true;
        } else { return false; }
    }
    /// Reference from QUIC RFC. https://quicwg.org/base-drafts/draft-ietf-quic-recovery.html
    pub fn cc_is_in_congestion_recovery(&self, time: Instant) -> bool {
        if self.congestion_recovery_start_time == None { return false; }
        time <= self.congestion_recovery_start_time.unwrap()
    }
    pub fn cc_on_packet_sent(&mut self, packet_size: usize) {
        self.bytes_in_flight += packet_size;
    }
    pub fn cc_on_packet_acked(&mut self, acked_packet: &SentPacket) {
        let result = self.sent_ack_largest.get(&acked_packet.packet_num);
        if result != None {
            self.ack_starting_packet_num = cmp::max(self.ack_starting_packet_num, result.unwrap() + 1);
        }
        self.bytes_in_flight -= acked_packet.size;
        if self.cc_is_in_congestion_recovery(acked_packet.time_sent) {
            if acked_packet.time_sent > self.congestion_recovery_start_time.unwrap() {
                debug!("Out of congestion recovery.");
                self.congestion_recovery_start_time = None;
            }
            return;
        }
        if acked_packet.size <= 20 { return; }
        if self.congestion_window < self.slow_start_threshold {
            // in slow start
            self.congestion_window += acked_packet.size;
            debug!("In slow start, increased congestion window to {}", self.congestion_window);
        } else {
            self.congestion_window = (self.congestion_window as f64 * ( 1.0 + (( self.get_cubic_increase() - self.congestion_window as f64 ) / self.congestion_window as f64))) as usize;
            debug!("In AIMD, increased congestion window to {}", self.congestion_window);
        }
    }
    pub fn cc_on_packet_lost(&mut self, lost_packet: &SentPacket) {
        self.bytes_in_flight -= lost_packet.size;
        self.sent_largest_lost = cmp::max(self.sent_largest_lost, lost_packet.packet_num);
        self.congestion_event(lost_packet.time_sent);
    }
    pub fn update_RTT(&mut self, mut ack_delay: u64) {
        if self.max_RTT == 0 {
            self.min_RTT = self.latest_RTT;
            self.max_RTT = self.latest_RTT;
            self.RTT_variance = self.latest_RTT / 2;
            return;
        }
        self.min_RTT = cmp::min(self.min_RTT, self.latest_RTT);
        // Limit ack_delay by max_ack_delay
        ack_delay = cmp::min(ack_delay, Duration::from_millis(2).as_nanos() as u64);
        // Adjust for ack delay if plausible.
        let adjusted_RTT = if self.latest_RTT > self.min_RTT + ack_delay { self.latest_RTT - ack_delay } else { self.latest_RTT };

        self.RTT_variance = (3.0 / 4.0 * self.RTT_variance as f64 + 1.0 / 4.0 * (self.smoothed_RTT as i64 - adjusted_RTT as i64).abs() as f64) as u64;
        self.smoothed_RTT = (7.0 / 8.0 * self.smoothed_RTT as f64 + 1.0 / 8.0 * adjusted_RTT as f64) as u64;
        debug!("Updating RTT. latest RTT: {}, adjusted: {}, smoothed: {}, variance: {}", self.latest_RTT, adjusted_RTT, self.smoothed_RTT, self.RTT_variance);
    }
    pub fn detect_packet_lost(&mut self) {
        let lost_timeout = self.get_lost_timeout();
        let PTO = self.get_PTO();
        let mut PTO_triggered = false;
        let mut lost = Vec::new();
        let mut lost_packets = Vec::new();
        for (packet_num, sent_packet) in self.sent_packets.iter() {
            if sent_packet.is_ack_only { continue; }
            if packet_num < &self.sent_largest_ACKed {
                // Less than largest ACKed, Time / Reorder threshold
                if sent_packet.time_sent.elapsed().as_nanos() as u64 > lost_timeout || *packet_num < self.sent_largest_ACKed - 3 {
                    lost.push(packet_num.clone());
                }
            } else {
                // larger than largest ACKed, PTO timeout
                if sent_packet.time_sent.elapsed().as_nanos() as u64 > PTO {
                    PTO_triggered = true;
                    if self.established == false { lost.push(packet_num.clone()); }
                }
            }
        }
        for key in lost.iter() {
            let sent_packet = self.sent_packets.remove(key).unwrap();
            if self.sent_largest_lost < *key { self.sent_largest_lost = *key; }
            lost_packets.push(sent_packet);
        }
        if lost_packets.len() > 0 {
            self.on_packets_lost(lost_packets);
        }
        if PTO_triggered {
            if self.PTO_amount > 0 {
                if self.last_PTO_time.unwrap().elapsed().as_nanos() as u64 > PTO { self.on_PTO(PTO); }
            } else { self.on_PTO(PTO); }
        }
    }
    pub fn on_PTO(&mut self, PTO: u64) {
        debug!("PTO of {} triggered. PTO amount: {}", PTO, self.PTO_amount);
        self.last_PTO = PTO;
        self.last_PTO_time = Some(Instant::now());
        self.PTO_amount += 1;
        if self.PTO_amount == 4 {
            self.congestion_window = 14720;
        }
        if self.established == true { self.send_PTO(); }
    }
    pub fn on_packets_lost(&mut self, lost_packets: Vec<SentPacket>) {
        for lost_packet in lost_packets.iter() {
            debug!("Packet {:?} declared as lost.", lost_packet);
            self.cc_on_packet_lost(lost_packet);
            self.lost_packets.push_back(lost_packet.packet_num);
        }
    }
    pub fn resend_lost_packet_data(&mut self, data: &Vec<u8>) {
        while self.bytes_in_flight < self.congestion_window {
            if self.lost_packets.len() == 0 { return; }
            let lost_packet_num = self.lost_packets.pop_front().unwrap();
            let data_segment = self.send_state.sent_data.remove(&lost_packet_num);
            if data_segment != None {
                let data_segment = data_segment.unwrap();
                self.build_new_data_packet_from_segment(data, data_segment);
            }
            self.send_a_packet_in_queue();
        }
    }
    pub fn generate_close_frame(&self) -> Frame {
        Frame {
            frame_type: FrameType::CLOSE,
            frame_data: vec![0],
        }
    }
    pub fn send_close_packet(&mut self) {
        let mut packet = self.build_new_empty_packet();
        packet.frames.push(self.generate_close_frame());
        self.closing = Some(packet.header.packet_num);
        debug!("Sending Close packet.");
        self.send_packet(packet);
    }
    pub fn get_cubic_increase(&mut self) -> f64 {
        let time_since_last_congestion = self.last_congestion_event.unwrap().elapsed().as_nanos() as f64 * 0.000000001;
        debug!("Time elapsed: {}", time_since_last_congestion);
        let K = (self.max_congestion_window as f64 * 0.8 / 1472.0).cbrt();
        debug!("cubic root: {}", K);
        debug!("change: {}", ((time_since_last_congestion - K)).powf(3.0) * 8.0 );
        let W = ((time_since_last_congestion - K)).powf(3.0) * 8.0 + self.max_congestion_window as f64;
        debug!("W: {}",W);
        W
    }
    pub fn set_max_congestion_windows(&mut self) {
        if self.max_congestion_window < self.last_max_congestion_window {
            self.last_max_congestion_window = self.max_congestion_window;
            self.max_congestion_window = (self.max_congestion_window as f64 * (2.0 - 0.2) / 2.0) as usize;
        } else {
            self.last_max_congestion_window = self.max_congestion_window;
        }

    }
}
