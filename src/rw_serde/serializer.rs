//! Serializer backed by an [`std::io::Write`](Write) implementation.
//!
//! This module provides a Serde serializer that writes values directly to any
//! type implementing [`std::io::Write`](Write). It uses the same format as
//! [`bincode`](https://github.com/bincode-org/bincode) with little endian bytes and varints.
//!
//! Since serialization is performed directly on the output stream, no
//! intermediate buffer is required.
//!
//! # Example
//!
//! ```no_run
//! use std::fs::File;
//! use serde::Serialize;
//! use orengine_utils::rw_serde::RWSerializer;
//!
//! #[derive(Serialize)]
//! struct Person {
//!     id: u64,
//!     name: String,
//! }
//!
//! let file = File::create("person.bin")?;
//! let mut serializer = RWSerializer::new(file);
//!
//! Person {
//!     id: 1,
//!     name: "Alice".into(),
//! }
//! .serialize(&mut serializer)?;
//!
//! serializer.flush()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
use crate::varint::WriteVarInt;
use serde;
use serde::Serialize;
use std::fmt::{Debug, Display, Formatter};
use std::io::Write;

/// A streaming Serde serializer over any [`std::io::Write`](Write) destination.
///
/// Values are written sequentially using the crate's binary encoding.
pub struct RWSerializer<Dst: Write> {
    destination: Dst,
}

impl<Dst: Write> RWSerializer<Dst> {
    /// Creates a new serializer from a writer.
    pub fn new(destination: Dst) -> Self {
        Self { destination }
    }

    /// Consumes the serializer and returns the wrapped writer.
    pub fn into_inner(self) -> Dst {
        self.destination
    }

    /// Returns a mutable reference to the underlying writer.
    pub fn as_dst_mut(&mut self) -> &mut Dst {
        &mut self.destination
    }

    /// Returns a shared reference to the underlying writer.
    pub fn as_dst(&self) -> &Dst {
        &self.destination
    }

    /// Flushes buffered data to the underlying writer.
    ///
    /// This forwards directly to [`std::io::Write::flush`](Write::flush).
    pub fn flush(&mut self) -> Result<(), std::io::Error> {
        self.destination.flush()
    }
}

/// Errors that can occur while serializing values.
///
/// These errors represent either I/O failures or unsupported serialization
/// patterns encountered while producing the crate's binary format.
#[derive(Debug)]
pub enum SerializeError {
    IO(std::io::Error),
    SequenceWithoutLen,
    FieldSkipped,
    Custom(String),
}

impl Display for SerializeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IO(err) => write!(f, "failed to write: {err}"),
            Self::FieldSkipped => {
                write!(f, "`skip_field` should be unreachable for `Serializer`")
            }
            Self::SequenceWithoutLen => {
                write!(f, "the collection without known length was provided")
            }
            Self::Custom(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for SerializeError {}

impl serde::ser::Error for SerializeError {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Self::Custom(msg.to_string())
    }
}

impl<'s, Dst: Write> serde::Serializer for &'s mut RWSerializer<Dst> {
    type Ok = usize;
    type Error = SerializeError;
    type SerializeSeq = SerializerWithAcc<'s, Dst>;
    type SerializeTuple = SerializerWithAcc<'s, Dst>;
    type SerializeTupleStruct = SerializerWithAcc<'s, Dst>;
    type SerializeTupleVariant = SerializerWithAcc<'s, Dst>;
    type SerializeMap = SerializerWithAcc<'s, Dst>;
    type SerializeStruct = SerializerWithAcc<'s, Dst>;
    type SerializeStructVariant = SerializerWithAcc<'s, Dst>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        self.destination
            .write_all(&[u8::from(v)])
            .map_err(SerializeError::IO)?;

        Ok(1)
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.destination.write_varint(v).map_err(SerializeError::IO)
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.destination.write_varint(v).map_err(SerializeError::IO)
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.destination.write_varint(v).map_err(SerializeError::IO)
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.destination.write_varint(v).map_err(SerializeError::IO)
    }

    fn serialize_i128(self, v: i128) -> Result<Self::Ok, Self::Error> {
        self.destination.write_varint(v).map_err(SerializeError::IO)
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.destination.write_varint(v).map_err(SerializeError::IO)
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.destination.write_varint(v).map_err(SerializeError::IO)
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.destination.write_varint(v).map_err(SerializeError::IO)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.destination.write_varint(v).map_err(SerializeError::IO)
    }

    fn serialize_u128(self, v: u128) -> Result<Self::Ok, Self::Error> {
        self.destination.write_varint(v).map_err(SerializeError::IO)
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.destination
            .write_all(&v.to_le_bytes())
            .map_err(SerializeError::IO)?;

        Ok(size_of::<f32>())
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.destination
            .write_all(&v.to_le_bytes())
            .map_err(SerializeError::IO)?;

        Ok(size_of::<f64>())
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.destination
            .write_all(&(v as u32).to_le_bytes())
            .map_err(SerializeError::IO)?;

        Ok(size_of::<char>())
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        let written = self
            .destination
            .write_varint(v.len() as u64)
            .map_err(SerializeError::IO)?;
        self.destination
            .write_all(v.as_bytes())
            .map_err(SerializeError::IO)?;

        Ok(written + v.len())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.destination
            .write_varint(v.len() as u64)
            .map_err(SerializeError::IO)?;
        self.destination.write_all(v).map_err(SerializeError::IO)?;

        Ok(4 + v.len())
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_bool(false)
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.serialize_bool(true)?;

        Ok(1 + value.serialize(self)?)
    }

    #[inline]
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(0)
    }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Ok(0)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_u32(variant_index)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Ok(self.serialize_u32(variant_index)? + value.serialize(self)?)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        if let Some(len) = len {
            Ok(SerializerWithAcc {
                written: self
                    .destination
                    .write_varint(len as u64)
                    .map_err(SerializeError::IO)?,
                serializer: self,
            })
        } else {
            Err(SerializeError::SequenceWithoutLen)
        }
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(SerializerWithAcc {
            written: 0,
            serializer: self,
        })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(SerializerWithAcc {
            written: 0,
            serializer: self,
        })
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        let written = self
            .destination
            .write_varint(variant_index)
            .map_err(SerializeError::IO)?;

        Ok(SerializerWithAcc {
            written,
            serializer: self,
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        if let Some(len) = len {
            Ok(SerializerWithAcc {
                written: self
                    .destination
                    .write_varint(len as u64)
                    .map_err(SerializeError::IO)?,
                serializer: self,
            })
        } else {
            Err(SerializeError::SequenceWithoutLen)
        }
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(SerializerWithAcc {
            written: 0,
            serializer: self,
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        let written = self
            .destination
            .write_varint(variant_index)
            .map_err(SerializeError::IO)?;

        Ok(SerializerWithAcc {
            written,
            serializer: self,
        })
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

pub struct SerializerWithAcc<'s, Dst: Write> {
    serializer: &'s mut RWSerializer<Dst>,
    written: usize,
}

impl<Dst: Write> serde::ser::SerializeSeq for SerializerWithAcc<'_, Dst> {
    type Ok = usize;
    type Error = SerializeError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.written += value.serialize(&mut *self.serializer)?;

        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.written)
    }
}

impl<Dst: Write> serde::ser::SerializeTuple for SerializerWithAcc<'_, Dst> {
    type Ok = usize;
    type Error = SerializeError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.written += value.serialize(&mut *self.serializer)?;

        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.written)
    }
}

impl<Dst: Write> serde::ser::SerializeTupleStruct for SerializerWithAcc<'_, Dst> {
    type Ok = usize;
    type Error = SerializeError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.written += value.serialize(&mut *self.serializer)?;

        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.written)
    }
}

impl<Dst: Write> serde::ser::SerializeTupleVariant for SerializerWithAcc<'_, Dst> {
    type Ok = usize;
    type Error = SerializeError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.written += value.serialize(&mut *self.serializer)?;

        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.written)
    }
}

impl<Dst: Write> serde::ser::SerializeMap for SerializerWithAcc<'_, Dst> {
    type Ok = usize;
    type Error = SerializeError;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.written += key.serialize(&mut *self.serializer)?;

        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.written += value.serialize(&mut *self.serializer)?;

        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.written)
    }
}

impl<Dst: Write> serde::ser::SerializeStruct for SerializerWithAcc<'_, Dst> {
    type Ok = usize;
    type Error = SerializeError;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.written += value.serialize(&mut *self.serializer)?;

        Ok(())
    }

    fn skip_field(&mut self, _key: &'static str) -> Result<(), Self::Error> {
        Err(SerializeError::FieldSkipped)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.written)
    }
}

impl<Dst: Write> serde::ser::SerializeStructVariant for SerializerWithAcc<'_, Dst> {
    type Ok = usize;
    type Error = SerializeError;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.written += value.serialize(&mut *self.serializer)?;

        Ok(())
    }

    fn skip_field(&mut self, _key: &'static str) -> Result<(), Self::Error> {
        Err(SerializeError::FieldSkipped)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.written)
    }
}
