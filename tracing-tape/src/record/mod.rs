mod callsite;
pub use callsite::{field_type, CallsiteFieldRecord, CallsiteRecord};

mod event;
pub use event::{EventRecord, EventValueRecord};

mod span;
pub use span::{
    SpanCloseRecord, SpanEnterRecord, SpanExitRecord, SpanFollowsRecord, SpanOpenRecord,
    SpanValueRecord,
};
use zerocopy::{little_endian, AsBytes, FromBytes, FromZeroes, Unaligned};

pub mod record_kind {
    pub const NOOP: u8 = 0;
    pub const THREAD_NAME: u8 = 0x01;
    pub const CALLSITE: u8 = 0x08;
    pub const CALLSITE_FIELD: u8 = 0x09;

    pub const EVENT: u8 = 0x10;
    pub const EVENT_VALUE: u8 = 0x11;

    pub const SPAN_OPEN: u8 = 0x20;
    pub const SPAN_ENTER: u8 = 0x21;
    pub const SPAN_EXIT: u8 = 0x22;
    pub const SPAN_CLOSE: u8 = 0x23;
    pub const SPAN_VALUE: u8 = 0x24;
    pub const SPAN_FOLLOWS: u8 = 0x25;
}

#[derive(Debug, AsBytes, FromBytes, FromZeroes, Unaligned)]
#[repr(C)]
pub struct RecordHeader {
    pub kind: u8,
    pub len: little_endian::U16,
}

impl RecordHeader {
    pub fn new(kind: u8, len: u16) -> Self {
        RecordHeader {
            kind,
            len: len.into(),
        }
    }
}
