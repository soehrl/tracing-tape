use zerocopy::{little_endian, AsBytes, FromBytes, FromZeroes, Unaligned};

use super::{record_kind, RecordHeader};

#[derive(AsBytes, FromZeroes, FromBytes)]
#[repr(transparent)]
pub struct CallsiteKindAndLevel(u8);

impl CallsiteKindAndLevel {
    pub fn new(kind: tracing::metadata::Kind, level: tracing::Level) -> Self {
        todo!()
        // CallsiteKindAndLevel((kind as u8) | (level as u8))
    }
}

#[derive(Debug, AsBytes, FromZeroes, FromBytes, Unaligned)]
#[repr(C)]
pub struct CallsiteRecord {
    pub header: RecordHeader,
    pub field_count: little_endian::U16,
    pub name_len: little_endian::U16,
    pub target_len: little_endian::U16,
    pub module_path_len: little_endian::U16,
    pub file_len: little_endian::U16,
    pub line: little_endian::U32,
    pub id: little_endian::U64,
}

impl CallsiteRecord {
    pub fn new(
        size: u16,
        kind: tracing::metadata::Kind,
        level: tracing::Level,
        field_count: u16,
        name_len: u16,
        target_len: u16,
        module_path_len: u16,
        file_len: u16,
        line: u32,
        id: u64,
    ) -> Self {
        CallsiteRecord {
            header: RecordHeader::new(record_kind::CALLSITE, size),
            // reserved: 0,
            // kind_and_level: CallsiteKindAndLevel::new(kind, level),
            field_count: field_count.into(),
            name_len: name_len.into(),
            target_len: target_len.into(),
            module_path_len: module_path_len.into(),
            file_len: file_len.into(),
            line: line.into(),
            id: id.into(),
        }
    }
}

#[derive(Debug, AsBytes, FromZeroes, FromBytes, Unaligned)]
#[repr(C)]
pub struct CallsiteFieldRecord {
    pub header: RecordHeader,
    pub field_name_len: little_endian::U16,
    pub callsite_id: little_endian::U64,
    pub field_id: little_endian::U64,
}

impl CallsiteFieldRecord {
    pub fn new(field_name_len: u16, callsite_id: u64, field_id: u64) -> Self {
        CallsiteFieldRecord {
            header: RecordHeader::new(
                record_kind::CALLSITE_FIELD,
                std::mem::size_of::<Self>() as u16 + field_name_len,
            ),
            field_name_len: field_name_len.into(),
            callsite_id: callsite_id.into(),
            field_id: field_id.into(),
        }
    }
}

pub mod field_type {
    pub const BOOL: u8 = 0;
    pub const I64: u8 = 1;
    pub const U64: u8 = 2;
    pub const I128: u8 = 3;
    pub const U128: u8 = 4;
    pub const F64: u8 = 5;
    pub const STR: u8 = 6;
    pub const ERROR: u8 = 7;
}
