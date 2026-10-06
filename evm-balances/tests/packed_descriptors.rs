//! Check the committed package that GUI users actually load. `buffa` decodes
//! the standard descriptor fields; the real CLI decoder is also checked ad hoc.
use buffa::{UnknownFieldData, UnknownFields};
use std::collections::{BTreeMap, BTreeSet};

const PACKAGE: &[u8] = include_bytes!("../../spkg/evm-balances-v0.5.0.spkg");
const SQL_FILE: &str = "sf/substreams/sink/sql/v1/deprecated.proto";
const DATABASE_FILE: &str = "sf/substreams/sink/database/v1/database.proto";

fn bytes(fields: &UnknownFields, number: u32) -> impl Iterator<Item = &[u8]> {
    fields.iter().filter_map(move |field| match &field.data {
        UnknownFieldData::LengthDelimited(value) if field.number == number => Some(value.as_slice()),
        _ => None,
    })
}

fn string(fields: &UnknownFields, number: u32) -> &str {
    std::str::from_utf8(bytes(fields, number).next().expect("descriptor string field")).unwrap()
}

fn number(fields: &UnknownFields, field_number: u32) -> u64 {
    fields
        .iter()
        .find_map(|field| match field.data {
            UnknownFieldData::Varint(value) if field.number == field_number => Some(value),
            _ => None,
        })
        .expect("descriptor numeric field")
}

fn files() -> Vec<UnknownFields> {
    // Package.proto_files = 1; each element is a FileDescriptorProto.
    bytes(&UnknownFields::decode_from_slice(PACKAGE).unwrap(), 1)
        .map(|file| UnknownFields::decode_from_slice(file).unwrap())
        .collect()
}

fn collect_message_names(message: &[u8], parent: &str, names: &mut BTreeSet<String>) {
    let descriptor = UnknownFields::decode_from_slice(message).unwrap();
    let full_name = format!("{parent}.{}", string(&descriptor, 1));
    assert!(names.insert(full_name.clone()), "duplicate protobuf message {full_name}");
    // DescriptorProto.nested_type = 3.
    for nested in bytes(&descriptor, 3) {
        collect_message_names(nested, &full_name, names);
    }
}

#[test]
fn packed_messages_are_unique_and_sql_uses_the_canonical_path() {
    let mut paths = BTreeSet::new();
    let mut names = BTreeSet::new();
    for file in files() {
        // FileDescriptorProto: name = 1, package = 2, message_type = 4.
        let path = string(&file, 1);
        assert!(paths.insert(path.to_owned()), "duplicate protobuf file {path}");
        for message in bytes(&file, 4) {
            collect_message_names(message, string(&file, 2), &mut names);
        }
    }
    assert!(paths.contains(SQL_FILE));
    assert!(!paths.contains("sf/substreams/sink/sql/v1/services.proto"));
    assert!(names.contains("sf.substreams.sink.sql.v1.Service"));
    assert!(names.contains("sf.substreams.sink.database.v1.DatabaseChanges"));
}

#[test]
fn packed_database_fields_describe_the_emitted_version() {
    let file = files().into_iter().find(|file| string(file, 1) == DATABASE_FILE).unwrap();
    let field_message = bytes(&file, 4)
        .map(|message| UnknownFields::decode_from_slice(message).unwrap())
        .find(|message| string(message, 1) == "Field")
        .unwrap();
    // DescriptorProto.field = 2; FieldDescriptorProto name = 1, number = 3,
    // type = 5 (9 = string, 14 = enum), type_name = 6.
    let fields: BTreeMap<u64, UnknownFields> = bytes(&field_message, 2)
        .map(|field| {
            let descriptor = UnknownFields::decode_from_slice(field).unwrap();
            (number(&descriptor, 3), descriptor)
        })
        .collect();
    assert_eq!((string(&fields[&2], 1), number(&fields[&2], 5)), ("value", 9));
    assert_eq!((string(&fields[&4], 1), number(&fields[&4], 5)), ("update_op", 14));
    assert_eq!(string(&fields[&4], 6), ".sf.substreams.sink.database.v1.Field.UpdateOp");
    assert!(!fields.contains_key(&3));
}
