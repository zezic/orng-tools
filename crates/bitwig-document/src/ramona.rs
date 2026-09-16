//! A scanning reader for Ramona binary v2 objects.
//!
//! Only what identity work needs: the top-level fields of one object, with the
//! plaintext offset of every text and UUID value. Everything else is walked for
//! its length and discarded. Recording offsets is what lets an identity be
//! rewritten by splicing a same-length value into the decrypted section, with no
//! re-serialization of the document.

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::{Error, Result};

/// How a field is keyed on the wire. The metadata section names its fields; the
/// document body numbers them.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FieldKey {
    Name(String),
    Id(i32),
}

/// A captured value and where its payload starts in the decrypted section.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// `wide` records UTF-16 encoding, which a replacement has to preserve.
    Text { text: String, offset: usize, wide: bool },
    Uuid { uuid: Uuid, offset: usize },
}

pub type Fields = BTreeMap<FieldKey, Value>;

const TAG_STRING: u8 = 8;
const TAG_UUID: u8 = 21;

pub struct Scanner<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Scanner<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Scanner { data, pos: 0 }
    }

    /// Read the top-level object, returning its text and UUID fields.
    pub fn scan_object(&mut self) -> Result<Fields> {
        let class_id = self.i32()?;
        if class_id == 4 {
            self.skip_string()?;
        }
        let mut fields = Fields::new();
        loop {
            let next = self.i32()?;
            if next == 0 {
                return Ok(fields);
            }
            let key = if next == 1 {
                FieldKey::Name(self.read_string()?.0)
            } else {
                FieldKey::Id(next)
            };
            let tag = self.u8()?;
            match tag {
                TAG_STRING => {
                    let offset = self.pos;
                    let (text, wide) = self.read_string()?;
                    fields.insert(key, Value::Text { text, offset, wide });
                }
                TAG_UUID => {
                    let offset = self.pos;
                    let uuid = self.read_uuid()?;
                    fields.insert(key, Value::Uuid { uuid, offset });
                }
                other => self.skip_value(other)?,
            }
        }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or(Error::Truncated { at: self.pos })?;
        let slice = self.data.get(self.pos..end).ok_or(Error::Truncated { at: self.pos })?;
        self.pos = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }

    /// Length prefix shared by every counted structure. Negative or absurd
    /// counts are rejected here rather than becoming huge allocations.
    fn count(&mut self) -> Result<usize> {
        let n = self.i32()?;
        if n < 0 || n as usize > self.data.len() {
            return Err(Error::BadLength { at: self.pos - 4, len: n });
        }
        Ok(n as usize)
    }

    fn read_uuid(&mut self) -> Result<Uuid> {
        let bytes: [u8; 16] = self.take(16)?.try_into().unwrap();
        Ok(Uuid::from_bytes(bytes))
    }

    /// The high bit of the length prefix selects UTF-16BE over Latin-1.
    fn read_string(&mut self) -> Result<(String, bool)> {
        let raw = self.i32()? as u32;
        let wide = raw & 0x8000_0000 != 0;
        let len = (raw & 0x7FFF_FFFF) as usize;
        if len > self.data.len() {
            return Err(Error::BadLength { at: self.pos - 4, len: len as i32 });
        }
        if wide {
            let units: Vec<u16> = self
                .take(len * 2)?
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_be_bytes(*pair))
                .collect();
            Ok((String::from_utf16_lossy(&units), true))
        } else {
            Ok((self.take(len)?.iter().map(|&b| b as char).collect(), false))
        }
    }

    fn skip_string(&mut self) -> Result<()> {
        self.read_string().map(|_| ())
    }

    fn skip_value(&mut self, tag: u8) -> Result<()> {
        match tag {
            1 | 5 => self.take(1).map(|_| ()),                 // Byte, Bool
            2 => self.take(2).map(|_| ()),                     // Int16
            3 | 6 | 11 | 12 => self.take(4).map(|_| ()),       // Int32, Float, refs
            4 | 7 => self.take(8).map(|_| ()),                 // Int64, Double
            8 => self.skip_string(),
            9 => self.scan_object().map(|_| ()),
            10 => Ok(()),                                      // NullObject
            13 | 17 => self.skip_array(1),                     // ByteArray, BoolArray
            14 => self.skip_array(2),
            15 | 23 => self.skip_array(4),                     // Int32Array, FloatArray
            16 | 24 => self.skip_array(8),                     // Int64Array, DoubleArray
            18 => self.skip_object_list(),
            20 => self.skip_string_object_map(),
            21 | 22 => self.take(16).map(|_| ()),              // Uuid, Color
            25 => {
                let n = self.count()?;
                (0..n).try_for_each(|_| self.skip_string())
            }
            26 => self.skip_relative_ref(),
            other => Err(Error::UnknownTag { at: self.pos - 1, tag: other }),
        }
    }

    fn skip_array(&mut self, stride: usize) -> Result<()> {
        let n = self.count()?;
        self.take(n * stride).map(|_| ())
    }

    /// Items are discriminated by a leading i32; anything else is an inline
    /// object whose class id that discriminant already is.
    fn skip_object_list(&mut self) -> Result<()> {
        loop {
            match self.i32()? {
                3 => return Ok(()),
                0 => {}
                1 | 2 => {
                    self.take(4)?;
                }
                5 => self.skip_relative_ref()?,
                class_id => self.skip_object_body(class_id)?,
            }
        }
    }

    fn skip_string_object_map(&mut self) -> Result<()> {
        if self.u8()? == 0 {
            return Ok(());
        }
        loop {
            self.skip_string()?;
            self.scan_object()?;
            if self.u8()? == 0 {
                return Ok(());
            }
        }
    }

    fn skip_relative_ref(&mut self) -> Result<()> {
        match self.i32()? {
            1 => {
                self.take(4)?;
            }
            class_id => self.skip_object_body(class_id)?,
        }
        self.skip_string()
    }

    /// An object whose class id has already been consumed by the caller.
    fn skip_object_body(&mut self, class_id: i32) -> Result<()> {
        if class_id == 4 {
            self.skip_string()?;
        }
        loop {
            let next = self.i32()?;
            if next == 0 {
                return Ok(());
            }
            if next == 1 {
                self.skip_string()?;
            }
            let tag = self.u8()?;
            self.skip_value(tag)?;
        }
    }
}

/// Splice `replacement` over `len` bytes at `offset`, rejecting a length change.
///
/// Same-length is what keeps every other byte, and therefore every recorded
/// offset, valid.
pub fn splice(section: &mut [u8], offset: usize, len: usize, replacement: &[u8]) -> Result<()> {
    if replacement.len() != len {
        return Err(Error::LengthChanged { expected: len, got: replacement.len() });
    }
    section
        .get_mut(offset..offset + len)
        .ok_or(Error::Truncated { at: offset })?
        .copy_from_slice(replacement);
    Ok(())
}
