#![allow(clippy::read_zero_byte_vec)]

#[path = "../flatbuffer.rs"]
mod flatbuffer;
#[path = "../util.rs"]
mod util;
use flatbuffer::hex_flatbuffer::{finish_messages_buffer, Packet};
use flatbuffers::{FlatBufferBuilder, WIPOffset};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
//use base64::{engine::general_purpose::STANDARD as b64, Engine};
use dotenv::dotenv;
use once_cell::sync::OnceCell;
use sqlx::{mysql::MySqlPool, query, MySql, Pool};
use std::{env, time::Duration};

use crate::flatbuffer::hex_flatbuffer::{
    ErrorResponseBuilder, MessagesBuilder, PacketBuilder, PacketData,
};

static DB_CONNECTION: OnceCell<Mutex<Pool<MySql>>> = OnceCell::new();

#[tokio::main]
async fn main() {
    //Setup of DB and other
    dotenv().ok();
    let db_url =
        env::var("DATABASE_URL").expect("DATABASE_URL env var not set or in .env, please set it");
    DB_CONNECTION
        .set(Mutex::new(
            MySqlPool::connect(&db_url)
                .await
                .unwrap_or_else(|_| panic!("failed to connect to db {}", db_url)),
        ))
        .unwrap();
    let con = DB_CONNECTION.get().unwrap().blocking_lock();

    query!(
        "
    CREATE TABLE IF NOT EXISTS `HexDataStorage` (
        Pattern VARCHAR(256) COMMENT 'the pattern to lookup db info' NOT NULL,
        Data MEDIUMBLOB COMMENT 'the NBT data of the object' NOT NULL,
        Password TINYBLOB COMMENT 'the key to delete this data' NOT NULL,
        Deletion TIMESTAMP COMMENT 'The time when this data will be deleted' NOT NULL,
        PRIMARY KEY (Pattern)
    );"
    )
    .execute(&*con)
    .await
    .expect("tried to create db table (if it didn't exist)");

    let host_url = env::var("URL").unwrap_or("127.0.0.1:8080".to_owned());
    let tcp = TcpListener::bind(&host_url)
        .await
        .expect("failed to bind URL for hosting?");

    tokio::spawn(async move {
        loop {
            std::thread::sleep(Duration::from_secs(60 * 10)); //every 10 minutes we run a DB purge
            let con = DB_CONNECTION.get().unwrap().blocking_lock();
            match query!("DELETE FROM HexDataStorage WHERE Deletion < NOW()")
                .execute(&*con)
                .await
            {
                Ok(res) => println!("pruned DB {} rows affected", res.rows_affected()),
                Err(err) => println!("failed the prune command: {}", err),
            }
        }
    });

    loop {
        let (stream, _) = tcp.accept().await.unwrap();
        tokio::spawn(async move { handle_conn(stream).await });
    }
}

fn why_send_s2c_packets_to_server(responses: &mut Vec<WIPOffset<Packet<'_>>>) {
    let mut fbb = FlatBufferBuilder::new();
    let mut fbb1 = fbb.clone();
    let msg = fbb.create_string("do not send s2c packets to the server");
    let mut err_packet = PacketBuilder::new(&mut fbb);
    err_packet.add_data_type(PacketData::ErrorResponse);
    let mut err_data = ErrorResponseBuilder::new(&mut fbb1);
    err_data.add_id(400);
    err_data.add_other(msg);
    err_packet.add_data(err_data.finish().as_union_value());
    responses.push(err_packet.finish())
}

fn why_is_a_field_empty(responses: &mut Vec<WIPOffset<Packet<'_>>>) {
    let mut fbb = FlatBufferBuilder::new();
    let mut fbb1 = fbb.clone();
    let msg = fbb.create_string("please make sure to fill all fields");
    let mut err_packet = PacketBuilder::new(&mut fbb);
    err_packet.add_data_type(PacketData::ErrorResponse);
    let mut err_data = ErrorResponseBuilder::new(&mut fbb1);
    err_data.add_id(400);
    err_data.add_other(msg);
    err_packet.add_data(err_data.finish().as_union_value());
    responses.push(err_packet.finish())
}

async fn handle_conn(mut stream: TcpStream) {
    let mut buffer = vec![];
    loop {
        buffer.clear();
        let _ = stream
            .read(&mut buffer)
            .await
            .expect("failed to read socket data!");
        match flatbuffer::hex_flatbuffer::root_as_messages(&buffer) {
            Ok(messages) => match messages.packets() {
                Some(packets) => {
                    let mut responses: Vec<WIPOffset<Packet<'_>>> = vec![];
                    for packet in packets {
                        match packet.data_type() {
                            PacketData::DeleteSuccess => {
                                why_send_s2c_packets_to_server(&mut responses)
                            }
                            PacketData::ErrorResponse => {
                                why_send_s2c_packets_to_server(&mut responses)
                            }
                            PacketData::GetSuccess => {
                                why_send_s2c_packets_to_server(&mut responses)
                            }
                            PacketData::PutSuccess => {
                                why_send_s2c_packets_to_server(&mut responses)
                            }
                            PacketData::TryDelete => {}
                            PacketData::TryGet => {}
                            PacketData::TryPut => {}
                            PacketData::NONE => why_is_a_field_empty(&mut responses)
                            flatbuffer::hex_flatbuffer::PacketData(8_u8..=u8::MAX) => {
                                println!("client is sending packet types that dont exist, be very afraid")
                            }
                        }
                    }
                    let mut fbb = FlatBufferBuilder::new();
                    let mut fbb2 = FlatBufferBuilder::new();
                    let packets_fbb = fbb.create_vector(&responses);
                    let mut messages = MessagesBuilder::new(&mut fbb);
                    messages.add_packets(packets_fbb);
                    messages.add_version(1);
                    let wip = messages.finish();
                    finish_messages_buffer(&mut fbb2, wip);
                    let finished = fbb2.finished_data();
                    let _ = stream.write(finished).await;
                }
                None => {
                    println!("why send a message if you aren't gonna send any packets!")
                }
            },
            Err(invalid) => {
                println!("recieved invalid packet! {}", invalid);
                let mut fbb = FlatBufferBuilder::new();
                let mut fbb1 = fbb.clone();
                let mut fbb2 = fbb.clone();
                let mut fbb3 = fbb.clone();
                let mut fbb4 = fbb.clone();
                let mut builder = MessagesBuilder::new(&mut fbb);
                builder.add_version(1);
                let mut err_packet = PacketBuilder::new(&mut fbb1);
                err_packet.add_data_type(PacketData::ErrorResponse);
                let mut err_resp = ErrorResponseBuilder::new(&mut fbb2);
                err_resp.add_id(400);
                err_resp.add_other(fbb3.create_string("invalid flatbuffer"));
                err_packet.add_data(err_resp.finish().as_union_value());
                let packets = [err_packet.finish()];
                builder.add_packets(fbb3.create_vector(&packets));
                let bytes = builder.finish();
                finish_messages_buffer(&mut fbb4, bytes);
                let _ = stream.write(fbb4.finished_data()).await;
            }
        }
    }
}
