#[path = "../flatbuffer.rs"]
mod flatbuffer;
#[path = "../util.rs"]
mod util;
use util::*;

//use base64::{engine::general_purpose::STANDARD as b64, Engine};
use dotenv::dotenv;
use quartz_nbt::{
    io::{write_nbt, Flavor},
    NbtCompound, NbtTag,
};
use rand::Rng;
use sqlx::{mysql::MySqlPool, query};
use std::env;

#[tokio::main]
async fn main() {
    dotenv().ok();
    let db_url =
        env::var("DATABASE_URL").expect("DATABASE_URL env var not set or in .env, please set it");
    let con = MySqlPool::connect(&db_url)
        .await
        .unwrap_or_else(|_| panic!("failed to connect to db {}", db_url));
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
    .execute(&con)
    .await
    .expect("tried to create db table (if it didn't exist)");

    match query!("DELETE FROM HexDataStorage WHERE Deletion < NOW()")
        .execute(&con)
        .await
    {
        Ok(res) => println!("pruned DB {} rows affected", res.rows_affected()),
        Err(err) => println!("failed the prune command: {}", err),
    }

    let mut rng = rand::thread_rng();
    let mut password = [0u8; 255];
    rng.fill(&mut password);

    let rand_iota: NbtCompound = sanatize_nbt(&NbtTag::Compound(generate_random_iota()))
        .try_into()
        .unwrap();
    let mut bytes = vec![];
    let res = write_nbt(&mut bytes, None, &rand_iota, Flavor::Uncompressed);
    if let Err(ohno) = res {
        panic!("failed to write nbt, {}", ohno);
    }
    let insert = query!(
        "INSERT INTO HexDataStorage (Pattern, Data, Password, Deletion) VALUES (?,?,?,?);",
        generate_random_sig(),
        bytes.as_slice(),
        password.as_ref(),
        time::OffsetDateTime::now_utc() + time::Duration::HOUR
    )
    .execute(&con)
    .await;
    if let Err(res) = insert {
        panic!("failed to add dummy data!!!, {:?}", res);
    }
}
