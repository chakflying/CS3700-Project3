# CS3700 Project 3 - Simple Transport Protocol

## High Level Approach

The project is structured such that both the sender and receiver uses a common module `protocol.rs`. The protocol is a simplified version of QUIC, implementing important features like data framing, PTO, ack ranges, etc. It features monotonically increasing packet number, such that packages determined to be lost will not be resent, instead the sender will look at the data which is stored in that lost packet, re-package it to send it again. This eliminates a lot of headaches like wrapping packet numbers and retransmitting too soon. The sender hence maintains a HashMap of packet numbers and it coresponding sent data ranges.

The receiver defaults to sending ACK every 2 packets, but will ACK every packet when packet reordering is detected. Each ACK packet contains alternating ACK ranges of present and missing packets, with only the largest ACKing packet stored as a full number. For example, if the receiver has new packets [201, 202, 204], it will respond with an ACK packet [204, 1, 1, 2], indicating 1 is received, follow by 1 missing, follow by 2 received. This feature of QUIC allow efficient ACK of large number of incoming packages, and provides reasonable redundency as some past information is provided.

In terms of congestion control, it uses a mix of New Reno's fast recovery and AIMD, as detailed in the QUIC spec. A recovery event is defined as the period between a packet is determined to be lost, and a new packet sent after this time is ACKed. The congestion window will only decrease once every recovery event, even though multiple packets may be lost. It also implements a crude bandwidth estimation by an estimated RTT and maximum congestion window ever achieved. If the current congestion window is not near the estimated bandwidth, congestion window will grow more quickly and lost event will decrease congestion window by a smaller fraction. RTT estimation is also used to better determine when a packet can be deemed lost. It uses a simple exponential moving average again as detailed in the QUIC spec. 

## Challenges faced

Rust proved to be a difficult language to do fast iteration on, as the rigidness of type conversions means there is a lot of boilerplate code. Situations where packet loss is near 50% or delay is greater than 500ms proved to be very challenging, as PTO is not designed to handle such a high latency. Difficulty in debugging is increased as the testing program provided does not display stdout of our program, such that we don't know why the program failed or if it exited normally at all. UDP being a stateless protocol also meant that closing a connection properly is very difficult.

## Testing

Code is tested locally first on Windows, then on the gordon machine. The optional -r command line argument generates random bytes as input data instead of using stdin, providing more convenient testing. Providing environment variable RUST_LOG=debug enables detailed logging of the sending and receiving status.

## External Libraries Used

integer-encoding: for variable length integer encodings, used in packet numbers
clap: command line argument parsing
chrono: time management
bitflags: efficient encoding of packet and frame types using bit fields
