#![feature(duration_constants)]

#[path = "../flatbuffer.rs"]
mod flatbuffer;
#[path = "../util.rs"]
mod util;
use flatbuffer::hex_flatbuffer::{
    finish_messages_buffer, DeleteSuccess, DeleteSuccessArgs, ErrorResponse, ErrorResponseArgs,
    FlatbufferMoment, GetSuccess, GetSuccessArgs, Packet, PacketArgs, PutSuccess, PutSuccessArgs,
};
use flatbuffers::{FlatBufferBuilder, WIPOffset};
use quartz_nbt::io::{read_nbt, write_nbt, Flavor};
use rand::Rng;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
//use base64::{engine::general_purpose::STANDARD as b64, Engine};
use dotenv::dotenv;
use once_cell::sync::OnceCell;
use sqlx::{mysql::MySqlPool, query, MySql, Pool};
use std::{env, net::SocketAddr, thread, time::Duration};
use tracing::{error, info, instrument, trace, warn};

use crate::{
    flatbuffer::hex_flatbuffer::{root_as_messages, Messages, MessagesArgs, PacketData},
    util::{sanatize_nbt, SanatizedNBTResult},
};

static DB_CONNECTION: OnceCell<Mutex<Pool<MySql>>> = OnceCell::new();

#[tokio::main]
async fn main() {
    let _ = tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .compact()
            .with_target(true)
            .finish(),
    );

    info!("starting server!");
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
    info!("db connected");
    let con = DB_CONNECTION.get().unwrap().lock().await;

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
    .expect("tried to create db table (if it didn't exist), something went wrong");
    drop(con);
    info!("table setup");
    let host_url = env::var("URL").unwrap_or("127.0.0.1:8080".to_owned());
    let tcp = TcpListener::bind(&host_url)
        .await
        .expect("failed to bind URL for hosting?");
    info!("tcp binded");
    tokio::spawn(async move { prune_db().await });

    loop {
        let (stream, addr) = tcp.accept().await.unwrap();
        tokio::spawn(async move {
            info!("spawn connection");
            handle_conn(stream, addr).await;
        });
    }
}

#[instrument]
async fn prune_db() {
    loop {
        std::thread::sleep(Duration::from_secs(60 * 10)); //every 10 minutes we run a DB purge
        info!("running a prune");
        let con = DB_CONNECTION.get().unwrap().lock().await;
        match query!("DELETE FROM HexDataStorage WHERE Deletion < NOW()")
            .execute(&*con)
            .await
        {
            Ok(res) => info!("pruned DB {} rows affected", res.rows_affected()),
            Err(err) => error!("failed the prune DB command: {}", err),
        }
        drop(con);
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
    info!("client sent client-bound packets to server");
    make_err_packet(responses, 400, "do not send s2c packets to the server");
}

fn why_is_a_field_empty(responses: &mut Vec<WIPOffset<Packet<'_>>>) {
    warn!("some field in request is empty");
    make_err_packet(responses, 400, "please make sure to fill all fields");
}

#[instrument(skip(stream))]
async fn handle_conn(mut stream: TcpStream, saddr: SocketAddr) {
    let mut buffer = vec![];
    loop {
        buffer.clear();
        let mut sbuf = [0u8; 1024];
        while root_as_messages(&buffer).is_err() {
            let _ = stream.read(&mut sbuf).await;
            buffer.extend_from_slice(&sbuf);
        }
        let messages = root_as_messages(&buffer).unwrap();
        if messages.version() == 0 {
            thread::sleep(Duration::MILLISECOND);
            continue;
        }
        info!("valid packet recieved");
        match messages.packets() {
            Some(packets) => {
                let mut fbb = FlatBufferBuilder::new();
                let mut responses: Vec<WIPOffset<Packet<'_>>> = vec![];
                info!("itterating over packets");
                for packet in packets {
                    info!("packet: {:?}", packet.data_type());
                    match packet.data_type() {
                        PacketData::DeleteSuccess => why_send_s2c_packets_to_server(&mut responses),
                        PacketData::ErrorResponse => why_send_s2c_packets_to_server(&mut responses),
                        PacketData::GetSuccess => why_send_s2c_packets_to_server(&mut responses),
                        PacketData::PutSuccess => why_send_s2c_packets_to_server(&mut responses),
                        PacketData::TryDelete => {
                            let td = packet.data_as_try_delete().unwrap();
                            match td.password() {
                                None => why_is_a_field_empty(&mut responses),
                                Some(password) => match td.pattern() {
                                    None => why_is_a_field_empty(&mut responses),
                                    Some(pattern) => {
                                        trace!("sanatizing pattern");
                                        let pat: String = pattern
                                            .chars()
                                            .filter(|c| "qweasd".contains(*c))
                                            .collect();
                                        trace!("locking db");
                                        let con = DB_CONNECTION.get().unwrap().lock().await;
                                        trace!("locked db");
                                        let res = query!("DELETE FROM HexDataStorage WHERE Pattern = ? AND Password = ?;",
                                                pat,&password.0[..]
                                            ).execute(&*con).await;
                                        drop(con);
                                        trace!("unlocked db");
                                        match res {
                                            Ok(_res) => {
                                                trace!("create packet");
                                                let dsa = DeleteSuccessArgs::default();
                                                let packet_args = PacketArgs {
                                                    data_type: PacketData::DeleteSuccess,
                                                    data: Some(
                                                        DeleteSuccess::create(&mut fbb, &dsa)
                                                            .as_union_value(),
                                                    ),
                                                };
                                                responses
                                                    .push(Packet::create(&mut fbb, &packet_args));
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
                        PacketData::TryGet => {
                            let tg_packet = packet.data_as_try_get().unwrap();
                            match tg_packet.pattern() {
                                None => why_is_a_field_empty(&mut responses),
                                Some(pattern) => {
                                    trace!("locking db");
                                    let con = DB_CONNECTION.get().unwrap().lock().await;
                                    trace!("locked db");
                                    let q = query!(
                                        "SELECT Data FROM HexDataStorage WHERE Pattern = ?;",
                                        pattern
                                    )
                                    .fetch_one(&*con)
                                    .await;
                                    drop(con);
                                    trace!("unlocked db");
                                    match q {
                                        Ok(res) => {
                                            trace!("creating packet");
                                            let gsargs = GetSuccessArgs {
                                                nbt: Some(fbb.create_vector(&res.Data)),
                                            };
                                            let pargs = PacketArgs {
                                                data_type: PacketData::GetSuccess,
                                                data: Some(
                                                    GetSuccess::create(&mut fbb, &gsargs)
                                                        .as_union_value(),
                                                ),
                                            };
                                            responses.push(Packet::create(&mut fbb, &pargs));
                                        }
                                        Err(ohno) => make_err_packet(
                                            &mut responses,
                                            500,
                                            ohno.to_string().as_str(),
                                        ),
                                    };
                                }
                            }
                        }
                        PacketData::TryPut => {
                            let tp = packet.data_as_try_put().unwrap();
                            match tp.nbt() {
                                None => why_is_a_field_empty(&mut responses),
                                Some(nbt) => match tp.pattern() {
                                    None => why_is_a_field_empty(&mut responses),
                                    Some(pat) => {
                                        trace!("sanatizing iota");
                                        let mut nbytes = nbt.bytes();
                                        let nbt = read_nbt(&mut nbytes, Flavor::Uncompressed);
                                        if let Err(ono) = nbt {
                                            warn!("nbt was invalid");
                                            make_err_packet(
                                                &mut responses,
                                                400,
                                                ono.to_string().as_str(),
                                            );
                                            continue;
                                        }
                                        let SanatizedNBTResult {
                                            consumed_entity,
                                            resultant_compound,
                                        } = sanatize_nbt(nbt.unwrap().0);
                                        let mut ser_nbt = vec![];
                                        if let Err(e) = write_nbt(
                                            &mut ser_nbt,
                                            None,
                                            &resultant_compound,
                                            Flavor::Uncompressed,
                                        ) {
                                            error!("failed to seralize nbt post-seralization");
                                            make_err_packet(
                                                &mut responses,
                                                500,
                                                format!(
                                                    "failure while re-seralizing sanatized nbt: {}",
                                                    e.to_string()
                                                )
                                                .as_str(),
                                            )
                                        };
                                        trace!("stripping pattern");
                                        let pat = pat
                                            .chars()
                                            .filter(|c| "qweasd".contains(*c))
                                            .collect::<String>();
                                        trace!("generating password");
                                        let mut password = [0u8; 255];
                                        {
                                            let mut rng = rand::thread_rng();
                                            rng.fill(&mut password);
                                        }
                                        trace!("obtaining db lock");
                                        let con = DB_CONNECTION.get().unwrap().lock().await;
                                        trace!("db locked");
                                        let q = query!("INSERT INTO HexDataStorage (Pattern, Data, Password, Deletion) VALUES (?,?,?,?)",
                                                pat,ser_nbt,&password[..],time::OffsetDateTime::now_utc() + time::Duration::HOUR
                                            ).execute(&*con).await;
                                        drop(con);
                                        trace!("unlocked db");
                                        match q {
                                            Ok(_resp) => {
                                                trace!("creating packet");
                                                let fbmoment = FlatbufferMoment::new(&password);
                                                let psargs = PutSuccessArgs {
                                                    password: Some(&fbmoment),
                                                    sanatized_entity: consumed_entity,
                                                };
                                                let pargs = PacketArgs {
                                                    data_type: PacketData::PutSuccess,
                                                    data: Some(
                                                        PutSuccess::create(&mut fbb, &psargs)
                                                            .as_union_value(),
                                                    ),
                                                };
                                                responses.push(Packet::create(&mut fbb, &pargs));
                                            }
                                            Err(ohno) => make_err_packet(
                                                &mut responses,
                                                500,
                                                &ohno.to_string(),
                                            ),
                                        }
                                    }
                                },
                            }
                        }
                        PacketData::NONE => why_is_a_field_empty(&mut responses),
                        flatbuffer::hex_flatbuffer::PacketData(8_u8..=u8::MAX) => {
                            warn!("client is sending packet types that dont exist, be very afraid");
                            make_err_packet(&mut responses, 400, "request type not supported")
                        }
                    }
                }
                info!("finished processing packets");
                let margs = MessagesArgs {
                    version: 1,
                    packets: Some(fbb.create_vector(&responses)),
                };
                let message = Messages::create(&mut fbb, &margs);
                finish_messages_buffer(&mut fbb, message);
                let finished = fbb.finished_data();
                info!("packet finalized, sending to client");
                let _ = stream.write(finished).await;
            }
            None => {
                warn!("why send a message if you aren't gonna send any packets!")
            }
        }
    }
}
