#![feature(cow_is_borrowed)]

mod magnet;
mod messages;

use messages::bencode::{FromBencode, ToBencode};
use messages::*;

use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::time::Duration;

fn grab_socket() -> Result<UdpSocket, std::io::Error> {
    let localhost = Ipv4Addr::new(0, 0, 0, 0);
    let socket = SocketAddrV4::new(localhost, 0);
    UdpSocket::bind(socket)
}

fn rand_buff<const N: usize>() -> [u8; N] {
    let mut buf = [0; N];
    getrandom::getrandom(&mut buf).unwrap();
    buf
}

fn ip_from_ping<'a>(msg: &'a KRPCMessage) -> Option<&'a [u8; 4]> {
    if let KRPCMessageDetails::Response(response) = &msg.message &&
        let KRPCResponse::Ping { ip: opt_ip, .. } = response &&
        let Some(ip) = opt_ip &&
        let messages::Ip::V4 { addr, ..} = ip {
        Some(addr)
    } else {
        None
    }
}

fn node_id(ip: &[u8; 4]) -> [u8; 20] {
    // Calculate proper node ID as specified in http://www.bittorrent.org/beps/bep_0042.html
    let mut out = rand_buff::<20>();
    let r = out[19] & 0x7;

    let mut hash_input: [u8; 4] = [0x03, 0x0f, 0x3f, 0xff];
    hash_input.iter_mut().zip(ip).for_each(|(a, b)| *a &= b);
    hash_input[0] |= r << 5;

    let crc = crc32c::crc32c(&hash_input);

    out[0] = ((crc >> 24) & 0xff) as u8;
    out[1] = ((crc >> 16) & 0xff) as u8;
    out[2] = (((crc >> 8) & 0xf8) as u8) | (out[2] & 0x07);

    out
}

fn bootstrap(socket: &UdpSocket) -> Result<(), Box<dyn std::error::Error>> {
    let mut buf = [0; 512];
    let mut transaction_id = rand_buff::<2>();
    let mut message_id = rand_buff::<20>();

    getrandom::getrandom(&mut transaction_id).map_err(|_| "Couldn't access random device")?;
    getrandom::getrandom(&mut message_id).map_err(|_| "Couldn't access random device")?;

    let ping = KRPCMessage {
        transaction_id: &transaction_id,
        message: KRPCMessageDetails::Query(KRPCQuery::Ping { id: &message_id }),
    }
    .to_bencode();
    let addr = "127.0.0.1:6881";
    socket.send_to(&ping, addr)?;
    let (number_of_bytes, _) = socket.recv_from(&mut buf)?;
    let filled_buf = &mut buf[..number_of_bytes];
    let message = KRPCMessage::from_bencode(filled_buf)?;
    if let Some(ip) = ip_from_ping(&message) {
        println!("Found IP address {:?}", ip);
        println!("Node ID Calculated: {:x?}", node_id(ip));
    }
    Ok(())
}

fn get_peers(socket: &UdpSocket, addr: &str) {
    let mut buf = [0; 512];

    let ping = KRPCMessage {
        transaction_id: b"aa",
        message: KRPCMessageDetails::Query(KRPCQuery::GetPeers {
            id: b"abcdefghij0123456789",
            info_hash: b"mnopqrstuvwxyz123456",
        }),
    }
    .to_bencode();
    socket.send_to(&ping, addr).unwrap();
    let number_of_bytes = socket.recv(&mut buf).expect("Didn't receive data");
    let filled_buf = &mut buf[..number_of_bytes];
    println!("Retrieved {:?}", KRPCMessage::from_bencode(filled_buf));
}

fn main() {
    match grab_socket() {
        Ok(socket) => {
            let addr = format!("{}", socket.local_addr().unwrap());
            println!("Allocated socket {}", addr);
            socket
                .set_read_timeout(Some(Duration::new(10, 0)))
                .expect("Can't set timout");
            if let Err(err) = bootstrap(&socket) {
                println!("Failed to bootstrap server: {}", err);
            }
            get_peers(&socket, &addr);
        }
        Err(e) => {
            println!("Failed to connect {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    // Test cases described in BEP 42
    #[test_case([124, 31, 75, 21], 1, [0x5f, 0xbf, 0xb8])]
    #[test_case([21, 75, 31, 124], 6, [0x5a, 0x3c, 0xe8])]
    #[test_case([65, 23, 51, 170], 6, [0xa5, 0xd4, 0x30])]
    #[test_case([84, 124, 73, 14], 1, [0x1b, 0x03, 0x20])]
    #[test_case([43, 213, 53, 83], 2, [0xe5, 0x6f, 0x68])]
    fn test_node_id(ip: [u8; 4], r: u8, crc: [u8; 3]) {
        // To make these tests faster the last 3 bits in the examples are ignored
        // this is as we would have to iterate until 2 random numbers matched.
        // Ignoring those last bits mean we just need to iterate until rand % 7
        // matches
        assert!(r <= 7);
        loop {
            let mut id = node_id(&ip);
            id[2] &= 0xf8;
            if (id[19] & 0x7) == r {
                assert_eq!(&id[0..3], crc);
                break;
            }
        }
    }
}
