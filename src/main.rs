#![feature(const_generics)]
#![feature(cow_is_borrowed)]

mod magnet;
mod messages;
use messages::bencode::FromBencode;

fn main() {
    let magnet1 = "magnet:?xt=urn:md5:c12fe1c06bba254a9dc9f519b335aa7c";
    println!("{:?}", magnet::parse(magnet1));
    let magnet2 = "magnet:?xt.1=urn:md5:c12fe1c06bba254a9dc9f519b335aa7c&xt.2=urn:md5:c12adec06bba254a9dc9f519b335aa7c";
    println!("{:?}", magnet::parse(magnet2));
    let magnet3 = "magnet:?xt=urn:md5:c12fe1c06bba254a9dc9f519b335aa7c&dn=Great+Speeches+-+Martin+Luther+King+Jr.+-+I+Have+A+Dream.mp3";
    println!("{:?}", magnet::parse(magnet3));
    let magnet4 = "magnet:?xt=urn:sha1:TXGCZQTH26NL6OUQAJJPFALHG2LTGBC7&dn=Great+Speeches+-+Martin+Luther+King+Jr.+-+I+Have+A+Dream.mp3";
    println!("{:?}", magnet::parse(magnet4));
    let magnet5 = "magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1367a88a";
    println!("{:?}", magnet::parse(magnet5));

    let krpc1 = messages::KRPCMessage::from_bencode("d1:t1:11:y1:ee");
    println!("{:?}", krpc1);

    let krpc2 = messages::KRPCMessage::from_bencode("d1:y1:e1:t0:3:abcd1:e1:f4:listl1:a2:xzeee");
    println!("{:?}", krpc2);
}
