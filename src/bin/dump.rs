use base64::{engine::general_purpose::STANDARD as b64, Engine};
use dotenv::dotenv;
use quartz_nbt::io::{read_nbt, Flavor};
use sqlx::{query, MySqlPool};
use std::env;
use time::UtcOffset;

#[tokio::main]
async fn main() {
    dotenv().ok();
    let db_url =
        env::var("DATABASE_URL").expect("DATABASE_URL env var not set or in .env, please set it");
    let con = MySqlPool::connect(&db_url)
        .await
        .unwrap_or_else(|_| panic!("failed to connect to db {}", db_url));
    let dat = query!("SELECT * FROM `HexDataStorage` ORDER BY Deletion;")
        .fetch_all(&con)
        .await
        .expect("failed to query db");
    let utc = UtcOffset::current_local_offset().unwrap();
    for record in dat.iter() {
        let mut iotab = &record.Data[..];
        println!(
            "pattern: {}\nto be deleted at: {}\nsnbt: {}\nkey: {}\n",
            record.Pattern,
            record.Deletion,
            read_nbt(&mut iotab, Flavor::Uncompressed)
                .unwrap()
                .0
                .to_snbt(),
            b64.encode(&record.Password[..])
        );
    }
}
