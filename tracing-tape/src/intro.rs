use zerocopy::{little_endian, AsBytes, FromBytes, FromZeroes};

/// The magic sequence identifying the file type.
pub type Magic = [u8; 8];

/// The magic sequence identifying the tapfile format.
pub const MAGIC: Magic = *b"TAPEFILE";

/// The version of the tapfile format.
#[derive(Debug, AsBytes, FromBytes, FromZeroes)]
#[repr(C)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
}

pub const VERSION: Version = Version { major: 0, minor: 0 };

/// The size of a chapter in the tapfile.
///
/// This represents the number of bytes in a chapter, i.e., the chunk size. This must be a power of
/// two, thus it is stored as a `u8` representing the exponent of the power of two.
#[derive(Debug, AsBytes, FromBytes, FromZeroes)]
#[repr(transparent)]
pub struct ChapterSize(pub u8);

impl Into<usize> for ChapterSize {
    fn into(self) -> usize {
        1 << self.0 as usize
    }
}

/// The introductory header of the tapfile.
#[derive(Debug, AsBytes, FromBytes, FromZeroes)]
#[repr(C)]
pub struct Intro {
    pub magic: [u8; 8],
    pub version: Version,
    pub chapter_size: ChapterSize,
    _padding: [u8; 5],
    pub timestamp_base: little_endian::I128,
}

impl Intro {
    pub fn new(chapter_size: u8, timestamp_base: i128) -> Self {
        Self {
            magic: MAGIC,
            version: VERSION,
            chapter_size: ChapterSize(chapter_size),
            _padding: [0; 5],
            timestamp_base: timestamp_base.into(),
        }
    }
}

#[test]
fn test_intro_size() {
    assert_eq!(std::mem::size_of::<Intro>(), 32);
}
