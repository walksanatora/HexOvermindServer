use quartz_nbt::{io::write_nbt, NbtCompound, NbtList};

fn main() {
    let mut garbage = NbtCompound::new();
    garbage.insert("hexcasting:type", "hexcasting:garbage");
    garbage.insert("hexcasting:data", NbtCompound::new());
    let mut test_ent = NbtCompound::new();
    test_ent.insert("hexcasting:type", "hexcasting:entity");
    //{name: '{"text":"walksanator"}', uuid: [I; 1583201733, 245647309, -1159122008, 372905589]}
    let mut data = NbtCompound::new();
    data.insert("name", "{\"text\":\"walksanator\"}");
    data.insert("uuid", vec![1583201733, 245647309, -1159122008, 372905589]);
    test_ent.insert("hexcasting:data", data);

    let mut test_list = NbtCompound::new();
    test_list.insert("hexcasting:type", "hexcasting:list");
    let mut nbt_list = NbtList::new();
    nbt_list.push(test_ent.clone());
    nbt_list.push(test_ent.clone());
    nbt_list.push(test_ent);
    nbt_list.push(garbage.clone());

    test_list.insert("hexcasting:data", nbt_list);

    println!("{}", test_list.to_snbt());
    let mut f = std::fs::File::create("test.nbt").unwrap();
    write_nbt(
        &mut f,
        None,
        &test_list,
        quartz_nbt::io::Flavor::Uncompressed,
    )
    .unwrap();
}
