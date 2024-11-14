use zerocopy::{little_endian, AsBytes, FromBytes, FromZeroes, Unaligned};

use super::{record_kind, RecordHeader};

pub mod parent_kind {
    pub const ROOT: u8 = 0;
    pub const CURRENT: u8 = 1;
    pub const EXPLICIT: u8 = 2;
}

#[derive(Debug, Clone, Copy, AsBytes, FromZeroes, FromBytes, Unaligned)]
#[repr(C)]
pub struct SpanOpenRecord {
    pub header: RecordHeader,
    pub id: little_endian::U64,
    pub parent_id: little_endian::U64,
    pub callsite_id: little_endian::U64,
    pub timestamp: little_endian::I64,
}

impl SpanOpenRecord {
    pub fn new(id: u64, parent_id: Option<u64>, callsite_id: u64, timestamp: i64) -> Self {
        SpanOpenRecord {
            header: RecordHeader::new(
                record_kind::SPAN_OPEN,
                std::mem::size_of::<SpanOpenRecord>() as u16,
            ),
            id: id.into(),
            parent_id: parent_id.unwrap_or(0).into(),
            callsite_id: callsite_id.into(),
            timestamp: timestamp.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, AsBytes, FromZeroes, FromBytes, Unaligned)]
#[repr(C)]
pub struct SpanOpenRecord2 {
    pub span_open_record: SpanOpenRecord,
    pub parent_kind: u8,
}

impl SpanOpenRecord2 {
    pub fn new(id: u64, parent_kind: u8, parent_id: u64, callsite_id: u64, timestamp: i64) -> Self {
        SpanOpenRecord2 {
            span_open_record: SpanOpenRecord {
                header: RecordHeader::new(
                    record_kind::SPAN_OPEN,
                    std::mem::size_of::<SpanOpenRecord2>() as u16,
                ),
                id: id.into(),
                parent_id: parent_id.into(),
                callsite_id: callsite_id.into(),
                timestamp: timestamp.into(),
            },
            parent_kind,
        }
    }
}

impl From<SpanOpenRecord> for SpanOpenRecord2 {
    fn from(record: SpanOpenRecord) -> Self {
        SpanOpenRecord2 {
            span_open_record: record,
            parent_kind: parent_kind::EXPLICIT,
        }
    }
}

#[derive(Debug, Clone, Copy, AsBytes, FromZeroes, FromBytes, Unaligned)]
#[repr(C)]
pub struct SpanCloseRecord {
    pub header: RecordHeader,
    pub id: little_endian::U64,
    pub timestamp: little_endian::I64,
}

impl SpanCloseRecord {
    pub fn new(id: u64, timestamp: i64) -> Self {
        SpanCloseRecord {
            header: RecordHeader::new(
                record_kind::SPAN_CLOSE,
                std::mem::size_of::<SpanCloseRecord>() as u16,
            ),
            id: id.into(),
            timestamp: timestamp.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, AsBytes, FromZeroes, FromBytes, Unaligned)]
#[repr(C)]
pub struct SpanEnterRecord {
    pub header: RecordHeader,
    pub id: little_endian::U64,
    pub timestamp: little_endian::I64,
    pub thread_id: little_endian::U64,
}

impl SpanEnterRecord {
    pub fn new(id: u64, timestamp: i64, thread_id: u64) -> Self {
        SpanEnterRecord {
            header: RecordHeader::new(
                record_kind::SPAN_ENTER,
                std::mem::size_of::<SpanEnterRecord>() as u16,
            ),
            id: id.into(),
            timestamp: timestamp.into(),
            thread_id: thread_id.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, AsBytes, FromZeroes, FromBytes, Unaligned)]
#[repr(C)]
pub struct SpanExitRecord {
    pub header: RecordHeader,
    pub id: little_endian::U64,
    pub timestamp: little_endian::I64,
}

impl SpanExitRecord {
    pub fn new(id: u64, timestamp: i64) -> Self {
        SpanExitRecord {
            header: RecordHeader::new(
                record_kind::SPAN_EXIT,
                std::mem::size_of::<SpanExitRecord>() as u16,
            ),
            id: id.into(),
            timestamp: timestamp.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, AsBytes, FromZeroes, FromBytes, Unaligned)]
#[repr(C)]
pub struct SpanValueRecord {
    pub header: RecordHeader,
    pub kind: u8,
    pub field_id: little_endian::U64,
    pub span_id: little_endian::U64,
}

impl SpanValueRecord {
    pub fn new(field_id: u64, kind: u8, value_len: usize, span_id: u64) -> Self {
        SpanValueRecord {
            field_id: field_id.into(),
            header: RecordHeader::new(
                record_kind::SPAN_VALUE,
                (std::mem::size_of::<Self>() + value_len) as u16,
            ),
            kind,
            span_id: span_id.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, AsBytes, FromZeroes, FromBytes, Unaligned)]
#[repr(C)]
pub struct SpanFollowsRecord {
    pub header: RecordHeader,
    pub span_id: little_endian::U64,
    pub follows_id: little_endian::U64,
}

impl SpanFollowsRecord {
    pub fn new(span_id: u64, follows_id: u64) -> Self {
        SpanFollowsRecord {
            header: RecordHeader::new(
                record_kind::SPAN_FOLLOWS,
                std::mem::size_of::<SpanFollowsRecord>() as u16,
            ),
            span_id: span_id.into(),
            follows_id: follows_id.into(),
        }
    }
}
