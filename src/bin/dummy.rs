#[path = "../flatbuffer.rs"]
mod flatbuffer;
#[path = "../util.rs"]
mod util;
use flatbuffer::hex_flatbuffer::{
    finish_messages_buffer, Messages, MessagesArgs, Packet, PacketArgs, PacketData, TryPut,
    TryPutArgs,
};
use flatbuffers::FlatBufferBuilder;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use util::*;

//use base64::{engine::general_purpose::STANDARD as b64, Engine};
use dotenv::dotenv;
use quartz_nbt::io::{write_nbt, Flavor};
use std::env;

use crate::flatbuffer::hex_flatbuffer::root_as_messages;

#[tokio::main]
async fn main() {
    dotenv().ok();

    let rand_iota = util::generate_random_iota();

    let mut bytes = vec![];
    let res = write_nbt(&mut bytes, None, &rand_iota, Flavor::Uncompressed);
    if let Err(ohno) = res {
        panic!("failed to write nbt, {}", ohno);
    }

    let mut fbb = FlatBufferBuilder::new();
    let pat = generate_random_sig();
    let tpargs = TryPutArgs {
        pattern: Some(fbb.create_string(pat.as_str())),
        nbt: Some(fbb.create_vector(bytes.as_slice())),
    };

    let pargs = PacketArgs {
        data_type: PacketData::TryPut,
        data: Some(TryPut::create(&mut fbb, &tpargs).as_union_value()),
    };
    let pack = Packet::create(&mut fbb, &pargs);
    let margs = MessagesArgs {
        version: 1,
        packets: Some(fbb.create_vector(&[pack])),
    };
    let msg = Messages::create(&mut fbb, &margs);
    finish_messages_buffer(&mut fbb, msg);
    let mut tcp = TcpStream::connect(env::var("URL").unwrap_or("127.0.0.1:8080".to_owned()))
        .await
        .unwrap();
    let mut buff: Vec<u8> = fbb.finished_data().into();
    buff.push(0x0);
    println!("trying to send `{}` `{}`", pat, rand_iota);
    let _ = tcp.write(&buff).await.unwrap();
    let mut b = vec![];
    println!("reading!");
    let _ = tcp.read_to_end(&mut b).await;
    println!("tcp {:?}", root_as_messages(&b));
}
