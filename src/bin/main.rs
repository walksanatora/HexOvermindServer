#![allow(clippy::read_zero_byte_vec)]

#[path = "../flatbuffer.rs"]
mod flatbuffer;
#[path = "../util.rs"]
mod util;
use flatbuffer::hex_flatbuffer::{
    finish_messages_buffer, DeleteSuccess, DeleteSuccessArgs, ErrorResponse, ErrorResponseArgs,
    Packet, PacketArgs,
};
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

use crate::flatbuffer::hex_flatbuffer::{Messages, MessagesArgs, PacketData};

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

fn make_err_packet(responses: &mut Vec<WIPOffset<Packet<'_>>>, id: u16, message: &str) {
    let mut fbb = FlatBufferBuilder::new();
    let err_datum = ErrorResponseArgs {
        id,
        other: Some(fbb.create_string(message)),
    };
    let err_data = ErrorResponse::create(&mut fbb, &err_datum);
    let packet_data = PacketArgs {
        data_type: PacketData::ErrorResponse,
        data: Some(err_data.as_union_value()),
    };
    let err_packet = Packet::create(&mut fbb, &packet_data);
    responses.push(err_packet);
}

fn why_send_s2c_packets_to_server(responses: &mut Vec<WIPOffset<Packet<'_>>>) {
    make_err_packet(responses, 400, "do not send s2c packets to the server");
}

fn why_is_a_field_empty(responses: &mut Vec<WIPOffset<Packet<'_>>>) {
    make_err_packet(responses, 400, "please make sure to fill all fields");
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
                            PacketData::TryDelete => {
                                let td = packet.data_as_try_delete().unwrap();
                                match td.password() {
                                    None => why_is_a_field_empty(&mut responses),
                                    Some(password) => match td.pattern() {
                                        None => why_is_a_field_empty(&mut responses),
                                        Some(pattern) => {
                                            let pat: String = pattern
                                                .chars()
                                                .filter(|c| {
                                                    vec!['q', 'w', 'e', 'a', 's', 'd'].contains(c)
                                                })
                                                .collect();
                                            let con = DB_CONNECTION.get().unwrap().blocking_lock();
                                            let res = query!("DELETE FROM HexDataStorage WHERE Pattern = ? AND Password = ?;",
                                                pat,&password.0[..]
                                            ).execute(&*con).await;
                                            drop(con);
                                            match res {
                                                Ok(_res) => {
                                                    let mut fbb = FlatBufferBuilder::new();
                                                    let dsa = DeleteSuccessArgs::default();
                                                    let packet_args = PacketArgs {
                                                        data_type: PacketData::DeleteSuccess,
                                                        data: Some(
                                                            DeleteSuccess::create(&mut fbb, &dsa)
                                                                .as_union_value(),
                                                        ),
                                                    };
                                                    responses.push(Packet::create(
                                                        &mut fbb,
                                                        &packet_args,
                                                    ));
                                                }
                                                Err(ohno) => make_err_packet(
                                                    &mut responses,
                                                    500,
                                                    ohno.to_string().as_str(),
                                                ),
                                            }
                                        }
                                    },
                                }
                            }
                            PacketData::TryGet => {}
                            PacketData::TryPut => {}
                            PacketData::NONE => why_is_a_field_empty(&mut responses),
                            flatbuffer::hex_flatbuffer::PacketData(8_u8..=u8::MAX) => {
                                println!("client is sending packet types that dont exist, be very afraid")
                            }
                        }
                    }
                    let mut fbb = FlatBufferBuilder::new();
                    let margs = MessagesArgs {
                        version: 1,
                        packets: Some(fbb.create_vector(&responses)),
                    };
                    let message = Messages::create(&mut fbb, &margs);
                    finish_messages_buffer(&mut fbb, message);
                    let finished = fbb.finished_data();
                    let _ = stream.write(finished).await;
                }
                None => {
                    println!("why send a message if you aren't gonna send any packets!")
                }
            },
            Err(invalid) => {
                let mut fbb = FlatBufferBuilder::new();
                let mut packets = vec![];
                make_err_packet(&mut packets, 400, "invalid flatbuffer");
                let encoded_packets = fbb.create_vector(&packets);
                let margs = MessagesArgs {
                    version: 1,
                    packets: Some(encoded_packets),
                };
                let message = Messages::create(&mut fbb, &margs);
                println!("recieved invalid packet! {}", invalid);
                finish_messages_buffer(&mut fbb, message);
                let _ = stream.write(fbb.finished_data()).await;
            }
        }
    }
}
