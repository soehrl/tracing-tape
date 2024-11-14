use zerocopy::{little_endian, AsBytes, FromBytes, FromZeroes, Unaligned};

use super::{record_kind, RecordHeader};

#[derive(Debug, Clone, Copy, AsBytes, FromZeroes, FromBytes, Unaligned)]
#[repr(transparent)]
pub struct CallsiteInfo(u8);

impl CallsiteInfo {
    pub fn new(kind: tracing::metadata::Kind, level: tracing::Level) -> Self {
        let kind = Self::kind_to_u8(kind);
        let level = Self::level_to_u8(level);
        CallsiteInfo(kind | level)
    }

    const LEVEL_MASK: u8 = 0b0000_0111;
    const LEVEL_SHIFT: u8 = 0;
    const KIND_MASK: u8 = 0b0000_0011;
    const KIND_SHIFT: u8 = 3;

    fn level_to_u8(level: tracing::Level) -> u8 {
        let level: u8 = match level {
            tracing::Level::TRACE => 0,
            tracing::Level::DEBUG => 1,
            tracing::Level::INFO => 2,
            tracing::Level::WARN => 3,
            tracing::Level::ERROR => 4,
        };
        level << Self::LEVEL_SHIFT
    }

    fn level_from_u8(level: u8) -> Option<tracing::Level> {
        match (level >> Self::LEVEL_SHIFT) & Self::LEVEL_MASK {
            0 => Some(tracing::Level::TRACE),
            1 => Some(tracing::Level::DEBUG),
            2 => Some(tracing::Level::INFO),
            3 => Some(tracing::Level::WARN),
            4 => Some(tracing::Level::ERROR),
            _ => None,
        }
    }

    fn kind_to_u8(kind: tracing::metadata::Kind) -> u8 {
        let mut result = if kind.is_event() {
            0
        } else {
            assert!(kind.is_span());
            1
        };
        if kind.is_hint() {
            result |= 2;
        }
        result << Self::KIND_SHIFT
    }

    fn kind_from_u8(kind: u8) -> Option<tracing::metadata::Kind> {
        let kind = (kind >> Self::KIND_SHIFT) & Self::KIND_MASK;
        match kind {
            0 => Some(tracing::metadata::Kind::EVENT),
            1 => Some(tracing::metadata::Kind::SPAN),
            2 => Some(tracing::metadata::Kind::EVENT.hint()),
            3 => Some(tracing::metadata::Kind::SPAN.hint()),
            _ => None,
        }
    }

    pub fn kind(&self) -> Option<tracing::metadata::Kind> {
        Self::kind_from_u8(self.0)
    }

    pub fn level(&self) -> Option<tracing::Level> {
        Self::level_from_u8(self.0)
    }
}

#[derive(Debug, Clone, Copy, AsBytes, FromZeroes, FromBytes, Unaligned)]
#[repr(C)]
pub struct CallsiteRecord {
    pub header: RecordHeader,
    pub info: CallsiteInfo,
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
            info: CallsiteInfo::new(kind, level),
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

#[derive(Debug, Clone, Copy, AsBytes, FromZeroes, FromBytes, Unaligned)]
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
