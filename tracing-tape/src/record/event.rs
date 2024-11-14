use zerocopy::{little_endian, AsBytes, FromBytes, FromZeroes, Unaligned};

use super::{record_kind, RecordHeader};

#[derive(Debug, Clone, Copy, AsBytes, FromZeroes, FromBytes, Unaligned)]
#[repr(C)]
pub struct EventRecord {
    pub header: RecordHeader,
    pub value_count: little_endian::U16,
    pub timestamp: little_endian::I64,
    pub callsite_id: little_endian::U64,
    pub thread_id: little_endian::U64,
}

impl EventRecord {
    pub fn new(value_count: u16, timestamp: i64, callsite_id: u64, thread_id: u64) -> Self {
        EventRecord {
            header: RecordHeader::new(record_kind::EVENT, std::mem::size_of::<Self>() as u16),
            value_count: value_count.into(),
            timestamp: timestamp.into(),
            callsite_id: callsite_id.into(),
            thread_id: thread_id.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, AsBytes, FromZeroes, FromBytes, Unaligned)]
#[repr(C)]
pub struct EventValueRecord {
    pub header: RecordHeader,
    pub kind: u8,
    pub field_id: little_endian::U64,
    pub thread_id: little_endian::U64,
}

impl EventValueRecord {
    pub fn new(field_id: u64, kind: u8, value_len: usize, thread_id: u64) -> Self {
        EventValueRecord {
            field_id: field_id.into(),
            header: RecordHeader::new(
                record_kind::EVENT_VALUE,
                (std::mem::size_of::<Self>() + value_len) as u16,
            ),
            kind,
            thread_id: thread_id.into(),
        }
    }
}
