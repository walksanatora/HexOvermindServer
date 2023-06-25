mod flatbuffer;
use base64::{engine::general_purpose::STANDARD as b64, Engine};
use dotenv::dotenv;
use rand::Rng;
use sqlx::{mysql::MySqlPool, query};
use std::env;

#[tokio::main]
async fn main() {
    println!("getting db url");
    dotenv().ok();
    let db_url =
        env::var("DATABASE_URL").expect("DATABASE_URL env var not set or in .env, please set it");
    let con = MySqlPool::connect(&db_url)
        .await
        .unwrap_or_else(|_| panic!("failed to connect to db {}", db_url));

    println!("creating table if it doesen't exist");
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
    println!("inserting test case");
    let mut rng = rand::thread_rng();
    let datum = rng.gen::<[u8; 32]>();
    let mut password = [0u8; 255];
    rng.fill(&mut password);
    let insert = query!(
        "INSERT INTO HexDataStorage (Pattern, Data, Password, Deletion) VALUES (?,?,?,?);",
        "wqwdae",
        datum.as_ref(),
        password.as_ref(),
        time::OffsetDateTime::now_utc() + time::Duration::HOUR
    )
    .execute(&con)
    .await;
    if let Ok(res) = insert {
        println!("affected: {}", res.rows_affected());
    } else {
        let err = insert.unwrap_err();
        println!("it failed!!!, {:?}", err);
    }
    println!("get all data");
    let dat = query!("SELECT * FROM `HexDataStorage`;")
        .fetch_all(&con)
        .await
        .expect("failed to query db");
    println!("read:");
    for record in dat.iter() {
        println!(
            "pattern: {}\npassword: {}\ndata: {}\nto be deleted at: {}\n",
            record.Pattern,
            b64.encode(record.Password.clone()),
            b64.encode(record.Data.clone()),
            record.Deletion
        );
        let q = query!(
            "DELETE FROM HexDataStorage WHERE Pattern = ?",
            record.Pattern
        )
        .execute(&con)
        .await;
        if let Ok(res) = q {
            println!("affected: {}", res.rows_affected());
        } else {
            let err = q.unwrap_err();
            println!("it failed!!!, {:?}", err);
        }
    }
}
