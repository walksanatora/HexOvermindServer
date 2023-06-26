#[path = "../flatbuffer.rs"]
mod flatbuffer;

//use base64::{engine::general_purpose::STANDARD as b64, Engine};
use dotenv::dotenv;
use quartz_nbt::{NbtCompound, NbtList, NbtTag};
use rand::Rng;
use sqlx::{mysql::MySqlPool, query};
use std::env;

fn generate_random_sig() -> String {
    let mut rng = rand::thread_rng();
    let chars = rng.gen_range(1..=32);
    let mut output = "".to_owned();
    for _ in 0..chars {
        let num = rng.gen_range(0..6);
        output.push(match num {
            0 => 'q',
            1 => 'w',
            2 => 'e',
            3 => 'a',
            4 => 's',
            5 => 'd',
            _ => unreachable!("random between 0..6, thats 012345, somehow got {}", num),
        });
    }
    output
}

fn sanatize_nbt(tag: &NbtTag) -> NbtTag {
    match tag {
        NbtTag::Compound(ct) => {
            NbtTag::Compound(
                if let Ok(iota_type) = ct.get::<_, &str>("hexcasting:type") {
                    match iota_type {
                        "hexcasting:list" => {
                            //this can contain other iotas so we gotta sanatize them
                            if let Ok(tag) = ct.get::<_, &NbtList>("hexcasting:data") {
                                let mut new_list = NbtList::new();
                                for iota in tag.iter() {
                                    new_list.push(sanatize_nbt(iota));
                                }
                                ct.insert("hexcasting:data", new_list);
                            } //if data is for some reason not a list, ¯\_(ツ)_/¯ Not my problem
                            ct
                        }
                        "hexcasting:entity" => {
                            ct.insert("hexcasting:type", "hexcasting:garbage");
                            ct
                        } //the type we want to specifically fuck over
                        "hextweaks:dict" => {
                            if let Ok(kv) = ct.get::<_, &NbtCompound>("hexcasting:data") {
                                let mut sanatized_keys = NbtCompound::new();
                                let mut sanatized_values = NbtCompound::new();
                                if let Ok(keys) = kv.get::<_, &NbtList>("k") {
                                    for iota in keys.iter() {
                                        sanatized_keys.push(sanatize_nbt(iota));
                                    }
                                };
                                if let Ok(values) = kv.get::<_, &NbtList>("hexcasting:data") {};
                                let mut new_kv = NbtCompound::new();
                                new_kv.insert("k", sanatized_keys);
                                new_kv.insert("v", sanatized_values);
                                ct.insert("hexcasting:data", new_kv);
                            }; //if data is for some reason not a compound, ¯\_(ツ)_/¯ Not my problem
                            ct
                        }
                        other => {
                            #[cfg(debug_assertions)]
                            println!("iota type {} does not have any setup sanatization", other);
                            ct
                        } //not a type that we filter for/can hold other types
                    }
                } else {
                    ct
                }
                .clone(),
            )
        }
        x => x.clone(),
    }
}

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
    let datum = rng.gen::<[u8; 32]>();
    let mut password = [0u8; 255];
    rng.fill(&mut password);
    let insert = query!(
        "INSERT INTO HexDataStorage (Pattern, Data, Password, Deletion) VALUES (?,?,?,?);",
        generate_random_sig(),
        datum.as_ref(),
        password.as_ref(),
        time::OffsetDateTime::now_utc() + time::Duration::HOUR
    )
    .execute(&con)
    .await;
    if let Err(res) = insert {
        panic!("failed to add dummy data!!!, {:?}", res);
    }
    let dat = query!("SELECT * FROM `HexDataStorage` ORDER BY Deletion;")
        .fetch_all(&con)
        .await
        .expect("failed to query db");
    for record in dat.iter() {
        println!(
            "pattern: {}\nto be deleted at: {}\n",
            record.Pattern, record.Deletion
        );
    }
}
