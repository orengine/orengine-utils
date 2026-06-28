//! Deserializer backed by an [`std::io::Read`](Read) implementation.
//!
//! This module provides a Serde deserializer that reads values directly from any
//! type implementing [`std::io::Read`](Read). It uses the same format as
//! [`bincode`](https://github.com/bincode-org/bincode) with little endian bytes and varints.
//!
//! The deserializer performs streaming reads and does not require the entire
//! input to be buffered in memory.
//!
//! # Example
//!
//! ```no_run
//! use std::fs::File;
//! use serde::Deserialize;
//! use orengine_utils::rw_serde::RWDeserializer;
//!
//! #[derive(Deserialize)]
//! struct Person {
//!     id: u64,
//!     name: String,
//! }
//!
//! let file = File::open("person.bin")?;
//! let mut deserializer = RWDeserializer::new(file);
//!
//! let person: Person = Person::deserialize(&mut deserializer)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
use crate::small_string::SmallString;
use crate::varint::ReadVarInt;
use serde::de::{
    self, DeserializeSeed, EnumAccess, IntoDeserializer, MapAccess, SeqAccess, VariantAccess,
    Visitor,
};
use smallvec::SmallVec;
use std::fmt::{Display, Formatter};
use std::io::Read;

/// A streaming Serde deserializer over any [`std::io::Read`](Read) source.
///
/// Values are read sequentially from the underlying reader using the crate's
/// binary encoding.
pub struct RWDeserializer<Src: Read> {
    source: Src,
}

impl<Src: Read> RWDeserializer<Src> {
    /// Creates a new deserializer from a reader.
    pub fn new(source: Src) -> Self {
        Self { source }
    }

    /// Consumes the deserializer and returns the wrapped reader.
    pub fn into_inner(self) -> Src {
        self.source
    }

    /// Returns a mutable reference to the underlying reader.
    pub fn as_sr_mut(&mut self) -> &mut Src {
        &mut self.source
    }

    /// Returns a shared reference to the underlying reader.
    pub fn as_src(&self) -> &Src {
        &self.source
    }
}

/// Errors that can occur while deserializing data.
///
/// These errors represent either I/O failures or malformed input that cannot be
/// decoded according to the crate's binary format.
#[derive(Debug)]
pub enum DeserializeError {
    IO(std::io::Error),
    InvalidBool(u8),
    InvalidChar(u32),
    IdentifierWasExpected,
    AttemptToSkip,
    AttemptToGetAny,
    Custom(String),
}

impl Display for DeserializeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IO(e) => write!(f, "{e}"),
            Self::InvalidBool(v) => write!(f, "invalid bool {v}"),
            Self::InvalidChar(v) => write!(f, "invalid char {v}"),
            Self::IdentifierWasExpected => write!(
                f,
                "an identifier was expected but the `Serializer` doesn't write it"
            ),
            Self::AttemptToSkip => write!(f, "attempt to skip, but it is impossible"),
            Self::AttemptToGetAny => write!(f, "attempt to get any, but it is impossible"),
            Self::Custom(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for DeserializeError {}

impl de::Error for DeserializeError {
    fn custom<T: Display>(msg: T) -> Self {
        Self::Custom(msg.to_string())
    }
}

impl<'de, Src: Read> de::Deserializer<'de> for &mut RWDeserializer<Src> {
    type Error = DeserializeError;

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let mut b = [0];

        self.source
            .read_exact(&mut b)
            .map_err(DeserializeError::IO)?;

        match b[0] {
            0 => visitor.visit_bool(false),
            1 => visitor.visit_bool(true),
            x => Err(DeserializeError::InvalidBool(x)),
        }
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i8(self.source.read_varint().map_err(DeserializeError::IO)?)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i16(self.source.read_varint().map_err(DeserializeError::IO)?)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i32(self.source.read_varint().map_err(DeserializeError::IO)?)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i64(self.source.read_varint().map_err(DeserializeError::IO)?)
    }

    fn deserialize_i128<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i128(self.source.read_varint().map_err(DeserializeError::IO)?)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u8(self.source.read_varint().map_err(DeserializeError::IO)?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u16(self.source.read_varint().map_err(DeserializeError::IO)?)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u32(self.source.read_varint().map_err(DeserializeError::IO)?)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u64(self.source.read_varint().map_err(DeserializeError::IO)?)
    }

    fn deserialize_u128<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u128(self.source.read_varint().map_err(DeserializeError::IO)?)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let mut buf = [0; 4];

        self.source
            .read_exact(&mut buf)
            .map_err(DeserializeError::IO)?;

        visitor.visit_f32(f32::from_le_bytes(buf))
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let mut buf = [0; 8];

        self.source
            .read_exact(&mut buf)
            .map_err(DeserializeError::IO)?;

        visitor.visit_f64(f64::from_le_bytes(buf))
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let mut buf = [0; 4];
        self.source
            .read_exact(&mut buf)
            .map_err(DeserializeError::IO)?;

        let c = u32::from_le_bytes(buf);

        char::from_u32(c).map_or_else(
            || Err(DeserializeError::InvalidChar(c)),
            |c| visitor.visit_char(c),
        )
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let len: u64 = self.source.read_varint().map_err(DeserializeError::IO)?;
        let buf: SmallString<1024> = SmallString::fill_from_reader(
            &mut self.source,
            usize::try_from(len).expect("Length overflow"),
        )
        .map_err(DeserializeError::IO)?;

        visitor.visit_str(&buf)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_string(visitor)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let len: u64 = self.source.read_varint().map_err(DeserializeError::IO)?;
        let mut buf =
            SmallVec::<u8, 1024>::with_capacity(usize::try_from(len).expect("Length overflow"));

        self.source
            .read_exact(&mut buf)
            .map_err(DeserializeError::IO)?;

        visitor.visit_bytes(&buf)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let len: u64 = self.source.read_varint().map_err(DeserializeError::IO)?;
        let mut buf = Vec::with_capacity(usize::try_from(len).expect("Length overflow"));

        #[allow(clippy::uninit_vec, reason = "We will imediatly fill it.")]
        unsafe {
            buf.set_len(buf.capacity())
        };

        #[allow(clippy::read_zero_byte_vec, reason = "False positive.")]
        self.source
            .read_exact(&mut buf)
            .map_err(DeserializeError::IO)?;

        visitor.visit_byte_buf(buf)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let exists: bool = self
            .source
            .read_varint::<u8>()
            .map_err(DeserializeError::IO)?
            > 0u8;

        if exists {
            visitor.visit_some(self)
        } else {
            visitor.visit_none()
        }
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let len: u64 = self.source.read_varint().map_err(DeserializeError::IO)?;

        visitor.visit_seq(Access {
            de: self,
            remaining: usize::try_from(len).expect("Length overflow"),
        })
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(Access {
            de: self,
            remaining: len,
        })
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(Access {
            de: self,
            remaining: len,
        })
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(Access {
            de: self,
            remaining: fields.len(),
        })
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let variant: u32 = self.source.read_varint().map_err(DeserializeError::IO)?;

        visitor.visit_enum(EnumAccessImpl { de: self, variant })
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let len: u64 = self.source.read_varint().map_err(DeserializeError::IO)?;

        visitor.visit_map(MapAccessImpl {
            de: self,
            remaining: usize::try_from(len).expect("Length overflow"),
        })
    }

    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(DeserializeError::AttemptToGetAny)
    }

    fn deserialize_identifier<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(DeserializeError::IdentifierWasExpected)
    }

    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(DeserializeError::AttemptToSkip)
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

struct Access<'a, Src: Read> {
    de: &'a mut RWDeserializer<Src>,
    remaining: usize,
}

impl<'de, Src: Read> SeqAccess<'de> for Access<'_, Src> {
    type Error = DeserializeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        if self.remaining == 0 {
            return Ok(None);
        }

        self.remaining -= 1;

        seed.deserialize(&mut *self.de).map(Some)
    }
}

struct EnumAccessImpl<'a, Src: Read> {
    de: &'a mut RWDeserializer<Src>,
    variant: u32,
}

impl<'de, Src: Read> EnumAccess<'de> for EnumAccessImpl<'_, Src> {
    type Error = DeserializeError;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let value = seed.deserialize(self.variant.into_deserializer())?;

        Ok((value, self))
    }
}

impl<'de, Src: Read> VariantAccess<'de> for EnumAccessImpl<'_, Src> {
    type Error = DeserializeError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(self.de)
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_tuple(self.de, len, visitor)
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_tuple(self.de, fields.len(), visitor)
    }
}

struct MapAccessImpl<'a, Src: Read> {
    de: &'a mut RWDeserializer<Src>,
    remaining: usize,
}

impl<'de, Src: Read> MapAccess<'de> for MapAccessImpl<'_, Src> {
    type Error = DeserializeError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        if self.remaining == 0 {
            return Ok(None);
        }

        seed.deserialize(&mut *self.de).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        self.remaining -= 1;

        seed.deserialize(&mut *self.de)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rw_serde::RWSerializer;
    use serde::{Deserialize, Serialize};
    use std::io::Cursor;

    fn serializer() -> RWSerializer<Vec<u8>> {
        RWSerializer::new(Vec::new())
    }

    #[test]
    fn deserialize_primitives() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct A {
            u8: u8,
            i8: i8,
            u16: u16,
            bool_2: bool,
            i16: i16,
            f64_2: f64,
            char_2: char,
            u32: u32,
            i32: i32,
            u64: u64,
            i64: i64,
            u8_2: u8,
            i8_2: i8,
            u16_2: u16,
            i16_2: i16,
            f32: f32,
            f64: f64,
            bool: bool,
            char: char,
            u32_2: u32,
            i32_2: i32,
            u64_2: u64,
            i64_2: i64,
            f32_2: f32,
        }

        let obj = A {
            u8: 1,
            i8: 2,
            u16: 3,
            bool_2: true,
            i16: 4,
            f64_2: 5.0,
            char_2: 'c',
            u32: 6,
            i32: 7,
            u64: 8,
            i64: 9,
            u8_2: 10,
            i8_2: 11,
            u16_2: 12,
            i16_2: 13,
            f32: 14.0,
            f64: 15.0,
            bool: false,
            char: 'd',
            u32_2: 16,
            i32_2: 17,
            u64_2: 2018,
            i64_2: 2019,
            f32_2: 20.0,
        };

        let mut ser = serializer();
        obj.serialize(&mut ser).unwrap();

        let buf = ser.into_inner();

        assert_eq!(buf.len(), 52);

        let mut de = RWDeserializer::new(Cursor::new(buf));
        let deserialized = A::deserialize(&mut de).unwrap();

        assert_eq!(obj, deserialized);
    }

    #[test]
    fn serialize_string() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct A {
            value: String,
        }

        let obj = A {
            value: "Hello, world!".into(),
        };

        let mut ser = serializer();
        obj.serialize(&mut ser).unwrap();

        let buf = ser.into_inner();

        assert_eq!(buf.len(), 14);

        let mut de = RWDeserializer::new(Cursor::new(buf));
        let out = A::deserialize(&mut de).unwrap();

        assert_eq!(obj, out);
    }

    #[test]
    fn serialize_vec() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct A {
            a: Vec<u32>,
            b: Vec<Vec<String>>,
        }

        let obj = A {
            a: vec![1, 2, 3, 4, 5],
            b: vec![
                vec![String::from("hello"), String::from("world")],
                vec![String::from("foo"), String::from("bar")],
            ],
        };

        let mut ser = serializer();
        obj.serialize(&mut ser).unwrap();

        let buf = ser.into_inner();

        assert_eq!(buf.len(), 6 + 23);

        let mut de = RWDeserializer::new(Cursor::new(buf));
        let out = A::deserialize(&mut de).unwrap();

        assert_eq!(obj, out);
    }

    #[test]
    fn serialize_some() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct A {
            a: Option<u32>,
            b: Option<u32>,
            c: Option<String>,
            d: Option<String>,
        }

        let obj = A {
            a: Some(42),
            b: None,
            c: None,
            d: Some("hello".into()),
        };

        let mut ser = serializer();
        obj.serialize(&mut ser).unwrap();

        let buf = ser.into_inner();

        assert_eq!(buf.len(), 11);

        let mut de = RWDeserializer::new(Cursor::new(buf));
        let out = A::deserialize(&mut de).unwrap();

        assert_eq!(obj, out);
    }

    #[test]
    fn serialize_nested_structs() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Inner {
            x: u32,
            y: String,
        }

        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Outer {
            a: Inner,
            b: Inner,
        }

        let obj = Outer {
            a: Inner {
                x: 42,
                y: "hello".into(),
            },
            b: Inner {
                x: 100,
                y: "world".into(),
            },
        };

        let mut ser = serializer();
        obj.serialize(&mut ser).unwrap();

        let buf = ser.into_inner();

        assert_eq!(buf.len(), 14);

        let mut de = RWDeserializer::new(Cursor::new(buf));
        let out = Outer::deserialize(&mut de).unwrap();

        assert_eq!(obj, out);
    }

    #[test]
    fn serialize_tuple_struct() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct A(u32, String, bool);

        let obj = A(123, "hello".into(), true);

        let mut ser = serializer();
        obj.serialize(&mut ser).unwrap();

        let buf = ser.into_inner();

        assert_eq!(buf.len(), 8);

        let mut de = RWDeserializer::new(Cursor::new(buf));
        let out = A::deserialize(&mut de).unwrap();

        assert_eq!(obj, out);
    }

    #[test]
    fn serialize_newtype_struct() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Id(u64);

        let obj = Id(999);

        let mut ser = serializer();
        obj.serialize(&mut ser).unwrap();

        let buf = ser.into_inner();

        assert_eq!(buf.len(), 2);

        let mut de = RWDeserializer::new(Cursor::new(buf));
        let out = Id::deserialize(&mut de).unwrap();

        assert_eq!(obj, out);
    }

    #[test]
    fn serialize_unit_struct() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Marker;

        let obj = Marker;

        let mut ser = serializer();
        obj.serialize(&mut ser).unwrap();

        let buf = ser.into_inner();

        assert_eq!(buf.len(), 0);

        let mut de = RWDeserializer::new(Cursor::new(buf));
        let out = Marker::deserialize(&mut de).unwrap();

        assert_eq!(obj, out);
    }

    #[test]
    fn serialize_enum() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        enum E {
            Unit,
            NewType(u32),
            Tuple(u32, String),
            Struct { id: u64, name: String },
        }

        let values = [
            (E::Unit, 1),
            (E::NewType(42), 2),
            (E::Tuple(1, "hello".into()), 8),
            (
                E::Struct {
                    id: 99,
                    name: "world".into(),
                },
                8,
            ),
        ];

        for (value, expected_size) in values {
            let mut ser = serializer();
            value.serialize(&mut ser).unwrap();

            let buf = ser.into_inner();

            assert_eq!(buf.len(), expected_size);

            let mut de = RWDeserializer::new(Cursor::new(buf));
            let out = E::deserialize(&mut de).unwrap();

            assert_eq!(value, out);
        }
    }

    #[test]
    fn serialize_array() {
        let obj = [1u16, 2, 3, 4, 5];

        let mut ser = serializer();
        obj.serialize(&mut ser).unwrap();

        let buf = ser.into_inner();

        assert_eq!(buf.len(), 5);

        let mut de = RWDeserializer::new(Cursor::new(buf));
        let out = <[u16; 5]>::deserialize(&mut de).unwrap();

        assert_eq!(obj, out);
    }

    #[test]
    fn serialize_hashmap() {
        use std::collections::HashMap;

        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct A {
            map: HashMap<String, u32>,
        }

        let mut map = HashMap::new();
        map.insert("one".into(), 1);
        map.insert("two".into(), 2);
        map.insert("three".into(), 3);

        let obj = A { map };

        let mut ser = serializer();
        obj.serialize(&mut ser).unwrap();

        let buf = ser.into_inner();

        assert_eq!(buf.len(), 1 + 5 + 5 + 7);

        let mut de = RWDeserializer::new(Cursor::new(buf));
        let out = A::deserialize(&mut de).unwrap();

        assert_eq!(obj, out);
    }

    #[test]
    fn serialize_btreemap() {
        use std::collections::BTreeMap;

        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct A {
            map: BTreeMap<String, u32>,
        }

        let mut map = BTreeMap::new();
        map.insert("one".into(), 1);
        map.insert("two".into(), 2);
        map.insert("three".into(), 3);

        let obj = A { map };

        let mut ser = serializer();
        obj.serialize(&mut ser).unwrap();

        let buf = ser.into_inner();

        assert_eq!(buf.len(), 1 + 5 + 5 + 7);

        let mut de = RWDeserializer::new(Cursor::new(buf));
        let out = A::deserialize(&mut de).unwrap();

        assert_eq!(obj, out);
    }

    #[test]
    fn serialize_hashset() {
        use std::collections::HashSet;

        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct A {
            values: HashSet<u32>,
        }

        let mut values = HashSet::new();
        values.insert(1);
        values.insert(2);
        values.insert(3);

        let obj = A { values };

        let mut ser = serializer();
        obj.serialize(&mut ser).unwrap();

        let buf = ser.into_inner();

        assert_eq!(buf.len(), 4);

        let mut de = RWDeserializer::new(Cursor::new(buf));
        let out = A::deserialize(&mut de).unwrap();

        assert_eq!(obj, out);
    }

    #[test]
    fn serialize_vecdeque() {
        use std::collections::VecDeque;

        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct A {
            values: VecDeque<u32>,
        }

        let obj = A {
            values: VecDeque::from(vec![1, 2, 3, 4]),
        };

        let mut ser = serializer();
        obj.serialize(&mut ser).unwrap();

        let buf = ser.into_inner();

        assert_eq!(buf.len(), 5);

        let mut de = RWDeserializer::new(Cursor::new(buf));
        let out = A::deserialize(&mut de).unwrap();

        assert_eq!(obj, out);
    }
}
