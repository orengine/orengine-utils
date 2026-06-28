//! A compact UTF-8 string backed by [`smallvec::SmallVec`].
//!
//! `SmallString` stores short strings inline without a heap allocation and
//! transparently spills to the heap when the inline capacity is exceeded.
//! It dereferences to `str`, making it convenient to use anywhere a string
//! slice is expected.
//!
//! The type implements `serde::Serialize` and `serde::Deserialize` as a
//! regular UTF-8 string.

use alloc::string::String;
#[cfg(not(feature = "no_std"))]
use core::fmt;
#[cfg(not(feature = "no_std"))]
use core::fmt::Formatter;
use core::ops::Deref;
#[cfg(not(feature = "no_std"))]
use serde::de::{Error, Visitor};
#[cfg(not(feature = "no_std"))]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use smallvec::SmallVec;

/// A UTF-8 string with configurable inline storage.
///
/// While the length is less or equal to `INLINE_SIZE`, the string is stored
/// inline on a stack. When the length exceeds this limit, the string spills
/// to the heap.
#[derive(Clone, Eq, PartialEq, Hash, PartialOrd, Ord, Debug)]
pub struct SmallString<const INLINE_SIZE: usize>(SmallVec<u8, INLINE_SIZE>);

impl<const INLINE_SIZE: usize> SmallString<INLINE_SIZE> {
    /// Creates an empty string.
    #[must_use]
    pub fn empty() -> Self {
        Self(SmallVec::new())
    }

    /// Reads a string of the specified length from a reader.
    ///
    /// # Errors
    ///
    /// Returns an error if the reader fails to read.
    #[cfg(not(feature = "no_std"))]
    pub(crate) fn fill_from_reader<R: std::io::Read>(
        mut reader: R,
        len: usize,
    ) -> Result<Self, std::io::Error> {
        let mut res = Self(SmallVec::new());

        res.0.resize(len, 0);
        reader.read_exact(&mut res.0)?;

        Ok(res)
    }

    /// Appends the provided UTF-8 bytes to the string.
    ///
    /// # Panics
    ///
    /// Panics if the resulting byte sequence is not valid UTF-8.
    ///
    /// This method is intended to be used only with valid UTF-8 data.
    pub fn extend_from_slice(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);

        // Preserve the invariant that the contents are always valid UTF-8.
        debug_assert!(core::str::from_utf8(&self.0).is_ok());
    }
}

impl<const INLINE_SIZE: usize> From<&str> for SmallString<INLINE_SIZE> {
    fn from(value: &str) -> Self {
        Self(value.as_bytes().into())
    }
}

impl<const INLINE_SIZE: usize> From<String> for SmallString<INLINE_SIZE> {
    fn from(value: String) -> Self {
        Self(value.into_bytes().into())
    }
}

impl<const INLINE_SIZE: usize> Deref for SmallString<INLINE_SIZE> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        core::str::from_utf8(&self.0).unwrap()
    }
}

#[cfg(not(feature = "no_std"))]
impl<const INLINE_SIZE: usize> Serialize for SmallString<INLINE_SIZE> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self)
    }
}

#[cfg(not(feature = "no_std"))]
impl<'de, const INLINE_SIZE: usize> Deserialize<'de> for SmallString<INLINE_SIZE> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SmallStringVisitor<const INLINE_SIZE: usize>;

        impl<const INLINE_SIZE: usize> Visitor<'_> for SmallStringVisitor<INLINE_SIZE> {
            type Value = SmallString<INLINE_SIZE>;

            fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
                formatter.write_str("a UTF-8 string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(SmallString::from(value))
            }
        }

        deserializer.deserialize_str(SmallStringVisitor::<INLINE_SIZE>)
    }

    fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SmallStringVisitorRef<'a, const INLINE_SIZE: usize>(
            &'a mut SmallString<INLINE_SIZE>,
        );

        impl<const INLINE_SIZE: usize> Visitor<'_> for SmallStringVisitorRef<'_, INLINE_SIZE> {
            type Value = ();

            fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
                formatter.write_str("a UTF-8 string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                self.0 .0.clear();
                self.0 .0.extend_from_slice(value.as_bytes());

                Ok(())
            }
        }

        deserializer.deserialize_str(SmallStringVisitorRef(place))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SmallString;
    use alloc::string::String;

    #[test]
    fn empty_string_is_empty() {
        let value = SmallString::<16>::empty();

        assert!(value.is_empty());
    }

    #[test]
    fn extend_appends_bytes() {
        let mut value = SmallString::<8>::from("hello");

        value.extend_from_slice(b" world");

        assert_eq!(&*value, "hello world");
    }

    #[test]
    fn from_string_and_str_are_equal() {
        let a = SmallString::<8>::from("example");
        let b = SmallString::<8>::from(String::from("example"));

        assert_eq!(a, b);
    }

    #[test]
    #[cfg(not(feature = "no_std"))]
    fn serde() {
        use crate::rw_serde::RWDeserializer;
        use crate::rw_serde::RWSerializer;
        use serde::{Deserialize, Serialize};
        use std::io::Cursor;

        let value = SmallString::<8>::from("small string");
        let mut ser = RWSerializer::new(Vec::new());

        value.serialize(&mut ser).unwrap();

        let buf = ser.into_inner();
        let mut de = RWDeserializer::new(Cursor::new(buf));
        let restored = SmallString::<8>::deserialize(&mut de).unwrap();

        assert_eq!(value, restored);
    }
}
