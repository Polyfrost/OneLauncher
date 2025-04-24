#![doc = include_str!("../README.md")]

pub use serde_json::json;

pub use de::{Deserializer, StreamDeserializer};

#[doc(inline)]
pub use de::{from_reader, from_slice, from_str};
/// Deserialize JSON data to a Rust data structure.
pub mod de {
    // See caveats in README.md
    pub use serde_json::de::{Deserializer, StreamDeserializer};

    use crate::Result;

    use std::io;

    pub use serde_json::de::{IoRead, Read, SliceRead, StrRead};

    use serde::{de::DeserializeOwned, Deserialize};

    /// Deserialize an instance of type `T` from a string of JSON text.
    ///
    /// Equivalent to [serde_json::from_str] but with errors extended with [serde_path_to_error].
    ///
    /// See [serde_json::from_str] for more documentation.
    pub fn from_str<'a, T>(s: &'a str) -> Result<T>
    where
        T: Deserialize<'a>,
    {
        let jd = &mut serde_json::Deserializer::from_str(s);

        serde_path_to_error::deserialize(jd)
    }

    /// Deserialize an instance of type `T` from an I/O stream of JSON.
    ///
    /// Equivalent to [serde_json::from_reader] but with errors extended with [serde_path_to_error].
    ///
    /// See [serde_json::from_reader] for more documentation.
    pub fn from_reader<R, T>(rdr: R) -> Result<T>
    where
        R: io::Read,
        T: DeserializeOwned,
    {
        let jd = &mut serde_json::Deserializer::from_reader(rdr);

        serde_path_to_error::deserialize(jd)
    }

    /// Deserialize an instance of type `T` from bytes of JSON text.
    ///
    /// Equivalent to [serde_json::from_slice] but with errors extended with [serde_path_to_error].
    ///
    /// See [serde_json::from_slice] for more documentation.
    pub fn from_slice<'a, T>(v: &'a [u8]) -> Result<T>
    where
        T: Deserialize<'a>,
    {
        let jd = &mut serde_json::Deserializer::from_slice(v);
		
        serde_path_to_error::deserialize(jd)
    }
}

#[doc(inline)]
pub use error::{Error, Result};
/// When serializing or deserializing JSON goes wrong.
pub mod error {
    pub use serde_json::error::Category;
    /// This type represents all possible errors that can occur when serializing or
    /// deserializing JSON data.
    pub type Error = serde_path_to_error::Error<serde_json::Error>;
    /// Alias for a `Result` with the error type `serde_json_path_to_error::Error`.
    pub type Result<T> = std::result::Result<T, Error>;
}

#[doc(inline)]
pub use ser::{
    to_string, to_string_pretty, to_vec, to_vec_pretty, to_writer, to_writer_pretty, Formatter,
    Serializer,
};
/// Serialize a Rust data structure into JSON data.
pub mod ser {
    use crate::Result;
    use serde::Serialize;

    static UTF8_ERROR: &str =
        "`serde_json` internally guarantees UTF8 and uses `String::from_utf8_unchecked`. \
        If this error throws, `serde_json` must have broken this guarantee";

    pub use serde_json::ser::{CharEscape, CompactFormatter, Formatter, PrettyFormatter};

    // See caveats in README.md
    pub use serde_json::ser::Serializer;

    /// Serialize the given data structure as JSON into the I/O stream.
    ///
    /// Equivalent to [serde_json::to_writer] but with errors extended with [serde_path_to_error].
    ///
    /// See [serde_json::to_writer] for more documentation.
    pub fn to_writer<W, T>(writer: W, value: &T) -> Result<()>
    where
        W: std::io::Write,
        T: ?Sized + Serialize,
    {
        let mut ser = Serializer::new(writer);
        serde_path_to_error::serialize(&value, &mut ser)
    }

    /// Serialize the given data structure as a JSON byte vector.
    ///
    /// Equivalent to [serde_json::to_vec] but with errors extended with [serde_path_to_error].
    ///
    /// See [serde_json::to_vec] for more documentation.
    pub fn to_vec<T>(value: &T) -> Result<Vec<u8>>
    where
        T: ?Sized + Serialize,
    {
        let mut bytes = Vec::new();

        to_writer(&mut bytes, value)?;

        Ok(bytes)
    }

    /// Serialize the given data structure as a String of JSON.
    ///
    /// Equivalent to [serde_json::to_string] but with errors extended with [serde_path_to_error].
    ///
    /// See [serde_json::to_string] for more documentation.
    pub fn to_string<T>(value: &T) -> Result<String>
    where
        T: ?Sized + Serialize,
    {
        let vec = to_vec(value)?;

        Ok(String::from_utf8(vec).expect(UTF8_ERROR))
    }

    /// Serialize the given data structure as pretty-printed JSON into the I/O
    /// stream.
    ///
    /// Equivalent to [serde_json::to_writer_pretty] but with errors extended with [serde_path_to_error].
    ///
    /// See [serde_json::to_writer_pretty] for more documentation.
    pub fn to_writer_pretty<W, T>(writer: W, value: &T) -> Result<()>
    where
        W: std::io::Write,
        T: ?Sized + Serialize,
    {
        let mut ser = Serializer::pretty(writer);
        serde_path_to_error::serialize(&value, &mut ser)
    }

    /// Serialize the given data structure as a pretty-printed JSON byte vector.
    ///
    /// Equivalent to [serde_json::to_vec_pretty] but with errors extended with [serde_path_to_error].
    ///
    /// See [serde_json::to_vec_pretty] for more documentation.
    pub fn to_vec_pretty<T>(value: &T) -> Result<Vec<u8>>
    where
        T: ?Sized + Serialize,
    {
        let mut bytes = Vec::new();

        to_writer_pretty(&mut bytes, value)?;

        Ok(bytes)
    }

    /// Serialize the given data structure as a pretty-printed String of JSON.
    ///
    /// Equivalent to [serde_json::to_string_pretty] but with errors extended with [serde_path_to_error].
    ///
    /// See [serde_json::to_string_pretty] for more documentation.
    pub fn to_string_pretty<T>(value: &T) -> Result<String>
    where
        T: ?Sized + Serialize,
    {
        let vec = to_vec_pretty(value)?;

        Ok(String::from_utf8(vec).expect(UTF8_ERROR))
    }
}

#[doc(inline)]
pub use map::Map;
/// A map of String to serde_json::Value.
///
/// See [serde_json::map] for more documentation.
pub mod map {
    pub use serde_json::map::{
        Entry, IntoIter, Iter, IterMut, Keys, Map, OccupiedEntry, VacantEntry, Values, ValuesMut,
    };
}

#[doc(inline)]
pub use value::{from_value, to_value, Number, Value};
/// The Value enum, a loosely typed way of representing any valid JSON value.
///
/// See [serde_json::value] for more documentation.
pub mod value {
    use serde::{de::DeserializeOwned, Serialize};

    pub use serde_json::value::{Index, Map, Number, Value};

    // See caveats in README.md
    pub use serde_json::value::Serializer;

    use crate::{Error, Result};

    /// Convert a `T` into `serde_json::Value` which is an enum that can represent
    ///
    /// Equivalent to [serde_json::to_value] but with errors extended with [serde_path_to_error].
    ///
    /// See [serde_json::to_value] for more documentation.
    pub fn to_value<T>(value: T) -> Result<Value>
    where
        T: Serialize,
    {
        let mut track = serde_path_to_error::Track::new();
        let ps = serde_path_to_error::Serializer::new(Serializer, &mut track);

        value.serialize(ps).map_err(|e| Error::new(track.path(), e))
    }

    /// Interpret a `serde_json::Value` as an instance of type `T`.
    ///
    /// Equivalent to [serde_json::from_value] but with errors extended with [serde_path_to_error].
    ///
    /// See [serde_json::from_value] for more documentation.
    pub fn from_value<T>(value: Value) -> Result<T>
    where
        T: DeserializeOwned,
    {
        serde_path_to_error::deserialize(value)
    }
}