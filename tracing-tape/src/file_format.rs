use serde::{Deserialize, Serialize, Serializer};

/// A magic byte sequence that identifies the file format.
const MAGIC: [u8; 8] = *b"TAPEFILE";

/// A magic byte sequence that identifies the file format.
///
/// This struct is zero-sized with special serialize/deserialize behavior. It serializes to the
/// byte sequence [`MAGIC`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Magic;

impl Serialize for Magic {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeTuple;
        let mut tuple = serializer.serialize_tuple(8)?;
        for &byte in &MAGIC {
            tuple.serialize_element(&byte)?;
        }
        tuple.end()
    }
}

impl<'de> Deserialize<'de> for Magic {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MagicVisitor;

        impl<'de> serde::de::Visitor<'de> for MagicVisitor {
            type Value = Magic;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a tuple of 8 bytes")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                let mut buf = [0; 8];
                for i in 0..8 {
                    buf[i] = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(i, &self))?;
                }

                if buf != MAGIC {
                    return Err(serde::de::Error::custom("invalid magic bytes"));
                }

                Ok(Magic)
            }
        }

        deserializer.deserialize_tuple(8, MagicVisitor)
    }
}

pub trait ToLittleEndian {
    type Array: AsRef<[u8]> + AsMut<[u8]> + Default;

    fn to_le_bytes(&self) -> Self::Array;
    fn from_le_bytes(array: Self::Array) -> Self;
}

macro_rules! impl_to_little_endian {
    ($($t:ty),*) => {
        $(
            impl ToLittleEndian for $t {
                type Array = [u8; std::mem::size_of::<$t>()];

                fn to_le_bytes(&self) -> Self::Array {
                    <$t>::to_le_bytes(*self)
                }

                fn from_le_bytes(array: Self::Array) -> Self {
                    <$t>::from_le_bytes(array)
                }
            }
        )*
    };
}
impl_to_little_endian!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedWidth<T: ToLittleEndian>(T);

impl<T: ToLittleEndian + Default> Default for FixedWidth<T> {
    fn default() -> Self {
        FixedWidth(T::default())
    }
}

impl<T: ToLittleEndian> From<T> for FixedWidth<T> {
    fn from(value: T) -> Self {
        FixedWidth(value)
    }
}

impl<T: ToLittleEndian> AsRef<T> for FixedWidth<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T: ToLittleEndian> std::ops::Deref for FixedWidth<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ToLittleEndian> std::ops::DerefMut for FixedWidth<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: ToLittleEndian> Serialize for FixedWidth<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeTuple;
        let size = std::mem::size_of::<T::Array>();
        let mut tuple = serializer.serialize_tuple(size)?;
        for &byte in self.0.to_le_bytes().as_ref() {
            tuple.serialize_element(&byte)?;
        }
        tuple.end()
    }
}

impl<'de, T: ToLittleEndian> Deserialize<'de> for FixedWidth<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FixedWidthVisitor<T: ToLittleEndian>(std::marker::PhantomData<T>);

        impl<'de, T: ToLittleEndian> serde::de::Visitor<'de> for FixedWidthVisitor<T> {
            type Value = FixedWidth<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(
                    formatter,
                    "a tuple of {} bytes",
                    std::mem::size_of::<T::Array>()
                )
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                let mut array = T::Array::default();
                for i in 0..std::mem::size_of::<T::Array>() {
                    array.as_mut()[i] = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(i, &self))?;
                }

                Ok(FixedWidth(T::from_le_bytes(array)))
            }
        }

        let size = std::mem::size_of::<T::Array>();
        deserializer.deserialize_tuple(size, FixedWidthVisitor::<T>(std::marker::PhantomData))
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum Version {
    V1,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Intro {
    pub magic: Magic,
    pub version: Version,
    pub timestamp_base: FixedWidth<i128>,
    pub chapter_len: FixedWidth<u32>,
}
impl Intro {
    pub fn new(chapter_len: u32, timestamp_base: time::OffsetDateTime) -> Self {
        Self {
            magic: Magic,
            version: Version::V1,
            timestamp_base: timestamp_base.unix_timestamp_nanos().into(),
            chapter_len: FixedWidth(chapter_len),
        }
    }
}
pub const SERIALIZED_INTRO_LEN: usize = 8 + 1 + 16 + 4;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ChapterSummary {
    pub min_timestamp: FixedWidth<u64>,
    pub max_timestamp: FixedWidth<u64>,
    /// The offset from the start of the chapter to the start of the first record.
    pub data_offset: FixedWidth<u32>,
    pub data_len: FixedWidth<u32>,
    pub metadata_count: FixedWidth<u32>,
    pub event_counts: [FixedWidth<u32>; 5],
}
pub const SERIALIZED_CHAPTER_SUMMARY_LEN: usize = 2 * 8 + 2 * 4 + 4 + 5 * 4;

#[test]
fn test_magic() {
    let mut slice = [0; 8];
    postcard::to_slice(&Magic, &mut slice).unwrap();
    assert_eq!(slice, [b'T', b'A', b'P', b'E', b'F', b'I', b'L', b'E']);
    assert!(postcard::from_bytes::<Magic>(&slice).is_ok());
}

#[test]
fn test_fixed_width() {
    let mut slice = [0; 8];
    postcard::to_slice(&FixedWidth(0x12345678u32), &mut slice).unwrap();
    assert_eq!(slice, [0x78, 0x56, 0x34, 0x12, 0, 0, 0, 0]);
    assert_eq!(
        postcard::from_bytes::<FixedWidth<u32>>(&slice).unwrap().0,
        0x12345678u32
    );
}

#[test]
fn test_file_header() {
    let mut slice = [0; SERIALIZED_INTRO_LEN];
    postcard::to_slice(
        &Intro {
            magic: Magic,
            version: Version::V1,
            timestamp_base: FixedWidth(0x12345678i128),
            chapter_len: FixedWidth(0x12345678u32),
        },
        &mut slice,
    )
    .unwrap();
    assert_eq!(
        slice,
        [
            b'T', b'A', b'P', b'E', b'F', b'I', b'L', b'E', 0, 0x78, 0x56, 0x34, 0x12, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0x78, 0x56, 0x34, 0x12
        ]
    );
    assert_eq!(
        postcard::from_bytes::<Intro>(&slice).unwrap(),
        Intro {
            magic: Magic,
            version: Version::V1,
            timestamp_base: FixedWidth(0x12345678i128),
            chapter_len: FixedWidth(0x12345678u32),
        }
    );
}
