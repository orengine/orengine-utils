//! Variable-length integer (varint) encoding and decoding.
//!
//! This module provides efficient serialization routines for integer types.
//!
//! Unsigned integers are encoded using a little-endian base-128 (LEB128-like)
//! variable-length representation. Small values occupy fewer bytes than larger
//! values.
//!
//! Signed integers are encoded using `ZigZag encoding` followed by unsigned
//! varint encoding. This efficiently stores small negative numbers.
//!
//! # Supported integer types
//!
//! - Unsigned: `u8`, `u16`, `u32`, `u64`, `u128`
//! - Signed: `i8`, `i16`, `i32`, `i64`, `i128`
//!
//! Convenience extension traits are also provided for all `Read` and `Write`
//! implementations.
//!
//! # Examples
//!
//! ```rust
//! use std::io::Cursor;
//! use orengine_utils::varint::{ReadVarInt, WriteVarInt};
//!
//! let mut buf = Vec::new();
//!
//! buf.write_varint(123u32).unwrap();
//! buf.write_varint(-42i32).unwrap();
//!
//! let mut cursor = Cursor::new(buf);
//!
//! let a: u32 = cursor.read_varint().unwrap();
//! let b: i32 = cursor.read_varint().unwrap();
//!
//! assert_eq!(a, 123);
//! assert_eq!(b, -42);
//! ```
use std::io::{self, Read, Write};

#[inline]
pub(crate) fn write_u8_to<W: Write + ?Sized>(n: u8, writer: &mut W) -> io::Result<usize> {
    writer.write_all(&[n])?;

    Ok(1)
}

#[inline]
pub(crate) fn read_u8_from<R: Read + ?Sized>(reader: &mut R) -> io::Result<u8> {
    let mut result = [0];

    reader.read_exact(&mut result)?;

    Ok(u8::from_le_bytes(result))
}

#[inline]
pub(crate) fn write_i8_to<W: Write + ?Sized>(n: i8, writer: &mut W) -> io::Result<usize> {
    writer.write_all(&n.to_le_bytes())?;

    Ok(1)
}

#[inline]
pub(crate) fn read_i8_from<R: Read + ?Sized>(reader: &mut R) -> io::Result<i8> {
    let mut result = [0];

    reader.read_exact(&mut result)?;

    Ok(result[0].cast_signed())
}

#[inline]
#[allow(clippy::cast_possible_truncation, reason = "False positive.")]
pub(crate) fn write_u128_to<W: Write + ?Sized>(mut n: u128, writer: &mut W) -> io::Result<usize> {
    let mut buf = [0u8; 19];
    let mut idx = 0;

    while n >= 0x80 {
        buf[idx] = (n as u8) | 0x80;
        n >>= 7;
        idx += 1;
    }

    buf[idx] = n as u8;
    idx += 1;

    writer.write_all(&buf[..idx])?;
    Ok(idx)
}

#[inline]
#[allow(clippy::cast_lossless, reason = "False positive.")]
pub(crate) fn read_u128_from<R: Read + ?Sized>(reader: &mut R) -> io::Result<u128> {
    let mut result = 0u128;
    let mut shift = 0;
    let mut buf = [0u8; 1];

    loop {
        reader.read_exact(&mut buf)?;
        let byte = buf[0];

        if shift >= 126 && (byte & 0x80) != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "varint too large for u128",
            ));
        }

        result |= ((byte & 0x7f) as u128) << shift;

        if byte & 0x80 == 0 {
            break;
        }

        shift += 7;
    }

    Ok(result)
}

macro_rules! impl_unsigned {
    ($ty:ty) => {
        paste::paste! {
            #[inline]
            #[allow(clippy::cast_lossless, reason = "It is generated code.")]
            #[allow(clippy::cast_possible_truncation, reason = "It is generated code.")]
            pub(crate) fn [<write_ $ty _to>]<W: Write + ?Sized>(
                value: $ty,
                writer: &mut W,
            ) -> io::Result<usize> {
                write_u128_to(value as u128, writer)
            }

            #[inline]
            #[allow(clippy::cast_lossless, reason = "It is generated code.")]
            #[allow(clippy::cast_possible_truncation, reason = "It is generated code.")]
            pub(crate) fn [<read_ $ty _from>]<R: Read + ?Sized>(
                reader: &mut R,
            ) -> io::Result<$ty> {
                let value = read_u128_from(reader)?;

                if value > <$ty>::MAX as u128 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        concat!("varint too large for ", stringify!($ty)),
                    ));
                }

                Ok(value as $ty)
            }

            pub trait [<Write $ty:camel>]: Write {
                fn [<write_ $ty>](
                    &mut self,
                    value: $ty,
                ) -> io::Result<usize> {
                    [<write_ $ty _to>](value, self)
                }
            }

            impl<T: Write> [<Write $ty:camel>] for T {}

            pub trait [<Read $ty:camel>]: Read {
                fn [<read_ $ty>](
                    &mut self,
                ) -> io::Result<$ty> {
                    [<read_ $ty _from>](self)
                }
            }

            impl<T: Read> [<Read $ty:camel>] for T {}
        }
    };
}

impl_unsigned!(u16);
impl_unsigned!(u32);
impl_unsigned!(u64);

#[inline]
#[allow(clippy::cast_sign_loss, reason = "It will be restored")]
fn zigzag_encode(v: i128) -> u128 {
    ((v << 1) ^ (v >> 127)) as u128
}

#[inline]
#[allow(clippy::cast_possible_wrap, reason = "It will be restored")]
fn zigzag_decode(v: u128) -> i128 {
    ((v >> 1) as i128) ^ (-((v & 1) as i128))
}

#[inline]
pub(crate) fn write_i128_to<W: Write + ?Sized>(n: i128, writer: &mut W) -> io::Result<usize> {
    write_u128_to(zigzag_encode(n), writer)
}

#[inline]
pub(crate) fn read_i128_from<R: Read + ?Sized>(reader: &mut R) -> io::Result<i128> {
    Ok(zigzag_decode(read_u128_from(reader)?))
}

macro_rules! impl_signed {
    ($ty:ty) => {
        paste::paste! {
            #[inline]
            #[allow(clippy::cast_lossless, reason = "It is generated code.")]
            #[allow(clippy::cast_possible_truncation, reason = "It is generated code.")]
            pub(crate) fn [<write_ $ty _to>]<W: Write + ?Sized>(
                value: $ty,
                writer: &mut W,
            ) -> io::Result<usize> {
                write_u128_to(zigzag_encode(value as i128), writer)
            }

            #[inline]
            #[allow(clippy::cast_lossless, reason = "It is generated code.")]
            #[allow(clippy::cast_possible_truncation, reason = "It is generated code.")]
            pub(crate) fn [<read_ $ty _from>]<R: Read + ?Sized>(
                reader: &mut R,
            ) -> io::Result<$ty> {
                let value = zigzag_decode(read_u128_from(reader)?);

                if value < <$ty>::MIN as i128 || value > <$ty>::MAX as i128 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        concat!("varint too large for ", stringify!($ty)),
                    ));
                }

                Ok(value as $ty)
            }
        }
    };
}

impl_signed!(i16);
impl_signed!(i32);
impl_signed!(i64);

/// A type that can be encoded and decoded as a variable-length integer.
///
/// This trait is implemented for all primitive integer types supported by this
/// module.
///
/// It is primarily intended for generic serialization code.
///
/// Most users should prefer using the [`ReadVarInt`] and [`WriteVarInt`]
/// extension traits.
pub trait VarInt: Sized {
    /// Writes the number as little-endian base-128 variable-length representation to the writer.
    fn write_as_varint_to<W: Write + ?Sized>(self, writer: &mut W) -> io::Result<usize>;
    /// Reads the number as little-endian base-128 variable-length representation from the reader.
    fn read_varint_from<R: Read + ?Sized>(reader: &mut R) -> io::Result<Self>;
}

macro_rules! impl_varint {
    ($($ty:ty),*) => {
        $(
            paste::paste! {
                impl VarInt for $ty {
                    #[inline]
                    fn write_as_varint_to<W: Write + ?Sized>(self, writer: &mut W) -> io::Result<usize> {
                        [<write_ $ty _to>](self, writer)
                    }

                    #[inline]
                    fn read_varint_from<R: Read + ?Sized>(reader: &mut R) -> io::Result<Self> {
                        [<read_ $ty _from>](reader)
                    }
                }
            }
        )*
    };
}

impl_varint!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128);

/// Extension trait for writing variable-length integers.
///
/// Implemented automatically for every type implementing [`Write`].
///
/// # Example
///
/// ```rust
/// use std::io::Cursor;
/// use orengine_utils::varint::WriteVarInt;
///
/// let mut writer = Cursor::new(Vec::new());
///
/// writer.write_varint(42u64).unwrap();
/// ```
pub trait WriteVarInt: Write {
    /// Writes [`VarInt`] to the writer.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::io::Cursor;
    /// use orengine_utils::varint::WriteVarInt;
    ///
    /// let mut writer = Cursor::new(Vec::new());
    ///
    /// writer.write_varint(42u64).unwrap();
    /// ```
    fn write_varint<T: VarInt>(&mut self, value: T) -> io::Result<usize> {
        value.write_as_varint_to(self)
    }
}

/// Extension trait for reading variable-length integers.
///
/// Implemented automatically for every type implementing [`Read`].
///
/// # Example
///
/// ```rust
/// use std::io::Cursor;
/// use orengine_utils::varint::{ReadVarInt, WriteVarInt};
///
/// let mut data = Vec::new();
/// data.write_varint(500u32).unwrap();
///
/// let mut reader = Cursor::new(data);
///
/// let value: u32 = reader.read_varint().unwrap();
///
/// assert_eq!(value, 500);
/// ```
pub trait ReadVarInt: Read {
    /// Reads a [`VarInt`] from the reader.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::io::Cursor;
    /// use orengine_utils::varint::{ReadVarInt, WriteVarInt};
    ///
    /// let mut data = Vec::new();
    /// data.write_varint(500u32).unwrap();
    ///
    /// let mut reader = Cursor::new(data);
    ///
    /// let value: u32 = reader.read_varint().unwrap();
    ///
    /// assert_eq!(value, 500);
    /// ```
    fn read_varint<T: VarInt>(&mut self) -> io::Result<T> {
        T::read_varint_from(self)
    }
}

impl<W: Write> WriteVarInt for W {}
impl<R: Read> ReadVarInt for R {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn roundtrip<T>(value: T)
    where
        T: VarInt + Copy + PartialEq + std::fmt::Debug,
    {
        let mut buf = Vec::new();

        value.write_as_varint_to(&mut buf).unwrap();

        let decoded = T::read_varint_from(&mut Cursor::new(buf)).unwrap();

        assert_eq!(value, decoded);
    }

    #[test]
    fn roundtrip_unsigned() {
        roundtrip(0u8);
        roundtrip(1u8);
        roundtrip(u8::MAX);

        roundtrip(0u16);
        roundtrip(127u16);
        roundtrip(128u16);
        roundtrip(u16::MAX);

        roundtrip(0u32);
        roundtrip(u32::MAX);

        roundtrip(0u64);
        roundtrip(u64::MAX);

        roundtrip(0u128);
        roundtrip(u128::MAX);
    }

    #[test]
    fn roundtrip_signed() {
        roundtrip(0i8);
        roundtrip(-1i8);
        roundtrip(i8::MIN);
        roundtrip(i8::MAX);

        roundtrip(0i16);
        roundtrip(-1i16);
        roundtrip(i16::MIN);
        roundtrip(i16::MAX);

        roundtrip(0i32);
        roundtrip(i32::MIN);
        roundtrip(i32::MAX);

        roundtrip(0i64);
        roundtrip(i64::MIN);
        roundtrip(i64::MAX);

        roundtrip(0i128);
        roundtrip(i128::MIN);
        roundtrip(i128::MAX);
    }

    #[test]
    fn encoding_size() {
        let mut buf = Vec::new();

        assert_eq!(write_u128_to(0, &mut buf).unwrap(), 1);

        buf.clear();
        assert_eq!(write_u128_to(127, &mut buf).unwrap(), 1);

        buf.clear();
        assert_eq!(write_u128_to(128, &mut buf).unwrap(), 2);

        buf.clear();
        assert_eq!(write_u128_to(16383, &mut buf).unwrap(), 2);

        buf.clear();
        assert_eq!(write_u128_to(16384, &mut buf).unwrap(), 3);
    }

    #[test]
    fn malformed_varint_is_rejected() {
        let bytes = [0xff; 19];

        let err = read_u128_from(&mut Cursor::new(bytes)).unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn overflow_for_smaller_type() {
        let mut buf = Vec::new();

        write_u32_to(70000, &mut buf).unwrap();

        let err = read_u16_from(&mut Cursor::new(buf)).unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn generic_traits_work() {
        let mut buf = Vec::new();

        buf.write_varint(12345u32).unwrap();
        buf.write_varint(-567i32).unwrap();

        let mut cursor = Cursor::new(buf);

        let a: u32 = cursor.read_varint().unwrap();
        let b: i32 = cursor.read_varint().unwrap();

        assert_eq!(a, 12345);
        assert_eq!(b, -567);
    }
}
