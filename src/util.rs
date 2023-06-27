#![allow(dead_code)]
use quartz_nbt::{NbtCompound, NbtList, NbtTag};
use rand::Rng;
pub fn generate_random_sig() -> String {
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

pub fn generate_random_iota() -> NbtCompound {
    let mut rng = rand::thread_rng();
    let result: u8 = rng.gen_range(1..=100);
    let mut tag = NbtCompound::new();
    println!("rng rolled a {}", result);
    match result {
        0..=10 => {
            tag.insert("hexcasting:type", "hexcasting:list");
            let mut list = NbtList::new();
            for _ in 0..rng.gen_range(1..=4) {
                list.push(generate_random_iota());
            }
            tag.insert("hexcasting:data", list);
        }
        11..=30 => {
            tag.insert("hexcasting:type", "hexcasting:string");
            tag.insert("hexcasting:data", "ohno \";DROP TABLE HexDataStorage;");
        }
        31..=50 => {
            tag.insert("hexcasting:type", "hexcasting:garbage");
            tag.insert("hexcasting:data", NbtCompound::new());
        }
        51..=70 => {
            tag.insert("hexcasting:type", "hexcasting:double");
            tag.insert(
                "hexcasting:data",
                NbtTag::Double(rng.gen_range(-100.0..100.0)),
            )
        }
        71..=95 => {
            //{name: '{"text":"walksanator"}', uuid: [I; 1583201733, 245647309, -1159122008, 372905589]}
            let mut data = NbtCompound::new();
            data.insert("name", "{\"text\":\"walksanator\"}");
            data.insert("uuid", vec![1583201733, 245647309, -1159122008, 372905589]);
            tag.insert("hexcasting:data", data);
            tag.insert("hexcasting:type", "hexcasting:entity")
        }
        96.. => {
            tag.insert("hexcasting:type", "hexcasting:list");
            let mut list = NbtList::new();
            for _ in 0..rng.gen_range(1..=4) {
                list.push(generate_random_iota());
            }
        }
    }
    tag
}

pub fn sanatize_nbt(tag: &NbtTag) -> NbtTag {
    match tag {
        NbtTag::Compound(cta) => {
            NbtTag::Compound(
                if let Ok(iota_type) = cta.get::<_, &str>("hexcasting:type") {
                    let mut ct = NbtCompound::new();
                    match iota_type {
                        "hexcasting:list" => {
                            //this can contain other iotas so we gotta sanatize them
                            let res = cta.get::<_, &NbtList>("hexcasting:data");
                            if let Ok(tag) = res {
                                let mut new_list = NbtList::new();
                                for iota in tag.iter() {
                                    new_list.push(sanatize_nbt(iota));
                                }

                                ct.insert("hexcasting:data", new_list);
                                ct.insert("hexcasting:type", "hexcasting:list")
                            } else {
                                println!("somehow the data is not a list!!! {}", res.unwrap_err());
                            } //if data is for some reason not a list, ¯\_(ツ)_/¯ Not my problem
                        }
                        "hexcasting:entity" => {
                            ct.insert("hexcasting:type", "hexcasting:garbage");
                            ct.insert("hexcasting:data", NbtCompound::new());
                        } //the type we want to specifically fuck over
                        "hextweaks:dict" => {
                            if let Ok(kv) = cta.get::<_, &NbtCompound>("hexcasting:data") {
                                let mut sanatized_keys = NbtList::new();
                                let mut sanatized_values = NbtList::new();
                                if let Ok(keys) = kv.get::<_, &NbtList>("k") {
                                    for iota in keys.iter() {
                                        sanatized_keys.push(
                                            if let NbtTag::Compound(datum) = iota {
                                                sanatize_nbt(&NbtTag::Compound(datum.clone()))
                                            } else {
                                                iota.clone()
                                            },
                                        );
                                    }
                                };
                                if let Ok(keys) = kv.get::<_, &NbtList>("v") {
                                    for iota in keys.iter() {
                                        sanatized_values.push(
                                            if let NbtTag::Compound(datum) = iota {
                                                sanatize_nbt(&NbtTag::Compound(datum.clone()))
                                            } else {
                                                iota.clone()
                                            },
                                        );
                                    }
                                };
                                let mut new_kv = NbtCompound::new();
                                new_kv.insert("k", sanatized_keys);
                                new_kv.insert("v", sanatized_values);
                                ct.insert("hexcasting:data", new_kv);
                                ct.insert("hexcasting:type", "hextweaks:dict");
                            }; //if data is for some reason not a compound, ¯\_(ツ)_/¯ Not my problem
                        }
                        other => {
                            #[cfg(debug_assertions)]
                            println!("iota type {} does not have any setup sanatization", other);
                            ct.insert("hexcasing:type", other);
                            ct.insert(
                                "hexcasting:data",
                                cta.get::<_, &NbtTag>("hexcasting:data").unwrap().clone(),
                            );
                        } //not a type that we filter for/can hold other types
                    };
                    ct
                } else {
                    cta.clone()
                },
            )
        }
        x => x.clone(),
    }
}
