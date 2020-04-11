#![feature(const_generics)]
#![feature(cow_is_borrowed)]

mod magnet;
mod messages;

use messages::bencode::{FromBencode, ToBencode};
use messages::*;

use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::time::Duration;

fn grab_socket() -> Result<UdpSocket, std::io::Error> {
    let localhost = Ipv4Addr::new(127, 0, 0, 1);
    let socket = SocketAddrV4::new(localhost, 0);
    UdpSocket::bind(socket)
}

fn ping(socket: &UdpSocket, addr: &str) {
    let mut buf = [0; 512];

    let ping = KRPCMessage {
        transaction_id: b"aa",
        message: KRPCMessageDetails::Query(KRPCQuery::Ping {
            id: b"abcdefghij0123456789",
        }),
    }.to_bencode();
    socket.send_to(&ping, addr).unwrap();
    let (number_of_bytes, _src_addr) =
                socket.recv_from(&mut buf).expect("Didn't receive data");
    let filled_buf = &mut buf[..number_of_bytes];
    println!("{:?}", filled_buf);
    println!(
        "Retrieved {:?}",
        KRPCMessage::from_bencode(filled_buf)
    );
}

fn get_peers(socket: &UdpSocket , addr: &str) {
    let mut buf = [0; 512];

    let ping = KRPCMessage {
        transaction_id: b"aa",
        message: KRPCMessageDetails::Query(KRPCQuery::GetPeers {
            id: b"abcdefghij0123456789",
            info_hash: b"mnopqrstuvwxyz123456"
        }),
    }.to_bencode();
    socket.send_to(&ping, addr).unwrap();
    let (number_of_bytes, _src_addr) =
                socket.recv_from(&mut buf).expect("Didn't receive data");
    let filled_buf = &mut buf[..number_of_bytes];
    println!("{:?}", filled_buf);
    println!(
        "Retrieved {:?}",
        KRPCMessage::from_bencode(filled_buf)
    );
}


fn main() {
    match grab_socket() {
        Ok(socket) => {
            println!("Allocated socket {}", socket.local_addr().unwrap());
            socket
                .set_read_timeout(Some(Duration::new(10, 0)))
                .expect("Can't set timout");
            ping(&socket, "127.0.0.1:6881");
            get_peers(&socket, "127.0.0.1:6881");

        }
        Err(e) => {
            println!("Failed to connect {}", e);
        }
    }
    println!("Terminated.");
}
