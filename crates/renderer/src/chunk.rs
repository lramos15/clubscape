//! Reader for the little-endian `CSRC` chunk container written by `tools/render-assets`.
//!
//! Layout: magic `CSRC`, u32 version, then repeated `[4-byte tag][u32 length][payload]`.
//! Unknown tags are ignored so exporters can add data without breaking older readers.

use std::collections::HashMap;

use crate::error::RenderError;

#[derive(Debug, Clone)]
pub struct Chunks<'a> {
    pub version: u32,
    chunks: HashMap<[u8; 4], &'a [u8]>,
}

impl<'a> Chunks<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, RenderError> {
        if data.len() < 8 || &data[0..4] != b"CSRC" {
            return Err(RenderError::Format("missing CSRC magic".into()));
        }
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let mut chunks = HashMap::new();
        let mut offset = 8usize;
        while offset < data.len() {
            if offset + 8 > data.len() {
                return Err(RenderError::Format("truncated chunk header".into()));
            }
            let tag: [u8; 4] = data[offset..offset + 4].try_into().expect("4 bytes");
            let len = u32::from_le_bytes(data[offset + 4..offset + 8].try_into().expect("4 bytes"))
                as usize;
            let start = offset + 8;
            let end = start
                .checked_add(len)
                .ok_or_else(|| RenderError::Format("chunk overflow".into()))?;
            if end > data.len() {
                return Err(RenderError::Format(format!(
                    "truncated chunk {}",
                    String::from_utf8_lossy(&tag)
                )));
            }
            chunks.insert(tag, &data[start..end]);
            offset = end;
        }
        Ok(Self { version, chunks })
    }

    pub fn has(&self, tag: &str) -> bool {
        self.chunks.contains_key(tag_bytes(tag).as_ref())
    }

    pub fn raw(&self, tag: &str) -> Result<&'a [u8], RenderError> {
        self.chunks
            .get(tag_bytes(tag).as_ref())
            .copied()
            .ok_or_else(|| RenderError::Format(format!("missing chunk {tag}")))
    }

    pub fn ints(&self, tag: &str) -> Result<Vec<i32>, RenderError> {
        let raw = self.raw(tag)?;
        if raw.len() % 4 != 0 {
            return Err(RenderError::Format(format!(
                "chunk {tag} is not i32 aligned"
            )));
        }
        Ok(raw
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| i32::from_le_bytes(*b))
            .collect())
    }

    pub fn ints_opt(&self, tag: &str) -> Result<Option<Vec<i32>>, RenderError> {
        if self.has(tag) {
            self.ints(tag).map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn floats(&self, tag: &str) -> Result<Vec<f32>, RenderError> {
        let raw = self.raw(tag)?;
        if raw.len() % 4 != 0 {
            return Err(RenderError::Format(format!(
                "chunk {tag} is not f32 aligned"
            )));
        }
        Ok(raw
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_bits(u32::from_le_bytes(*b)))
            .collect())
    }

    pub fn shorts(&self, tag: &str) -> Result<Vec<i16>, RenderError> {
        let raw = self.raw(tag)?;
        if raw.len() % 2 != 0 {
            return Err(RenderError::Format(format!(
                "chunk {tag} is not i16 aligned"
            )));
        }
        Ok(raw
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes(*b))
            .collect())
    }

    pub fn shorts_opt(&self, tag: &str) -> Result<Option<Vec<i16>>, RenderError> {
        if self.has(tag) {
            self.shorts(tag).map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn bytes(&self, tag: &str) -> Result<Vec<i8>, RenderError> {
        Ok(self.raw(tag)?.iter().map(|&b| b as i8).collect())
    }

    pub fn bytes_opt(&self, tag: &str) -> Result<Option<Vec<i8>>, RenderError> {
        if self.has(tag) {
            self.bytes(tag).map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn longs(&self, tag: &str) -> Result<Vec<i64>, RenderError> {
        let raw = self.raw(tag)?;
        if raw.len() % 8 != 0 {
            return Err(RenderError::Format(format!(
                "chunk {tag} is not i64 aligned"
            )));
        }
        Ok(raw
            .as_chunks::<8>()
            .0
            .iter()
            .map(|b| i64::from_le_bytes(*b))
            .collect())
    }

    /// Jagged i32 arrays: u32 count, then per row u32 length + values.
    pub fn jagged(&self, tag: &str) -> Result<Vec<Vec<i32>>, RenderError> {
        let ints = self.ints(tag)?;
        let mut rows = Vec::new();
        let mut cursor = 0usize;
        let count = *ints
            .first()
            .ok_or_else(|| RenderError::Format(format!("empty jagged {tag}")))?
            as usize;
        cursor += 1;
        for _ in 0..count {
            let len = *ints
                .get(cursor)
                .ok_or_else(|| RenderError::Format(format!("truncated jagged {tag}")))?
                as usize;
            cursor += 1;
            let end = cursor + len;
            if end > ints.len() {
                return Err(RenderError::Format(format!("truncated jagged row {tag}")));
            }
            rows.push(ints[cursor..end].to_vec());
            cursor = end;
        }
        Ok(rows)
    }

    pub fn jagged_opt(&self, tag: &str) -> Result<Option<Vec<Vec<i32>>>, RenderError> {
        if self.has(tag) {
            self.jagged(tag).map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn text(&self, tag: &str) -> Result<String, RenderError> {
        String::from_utf8(self.raw(tag)?.to_vec()).map_err(|e| RenderError::Format(e.to_string()))
    }
}

fn tag_bytes(tag: &str) -> [u8; 4] {
    let mut out = [b' '; 4];
    for (i, b) in tag.bytes().take(4).enumerate() {
        out[i] = b;
    }
    out
}
