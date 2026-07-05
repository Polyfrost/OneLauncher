use std::path::Path;

use crate::LauncherResult;
use crate::crypto::{normalize_hash, sha1_bytes};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileIdentity {
    pub sha1: String,
    pub cf_fingerprint: Option<u32>,
}

impl FileIdentity {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            sha1: sha1_bytes(bytes),
            cf_fingerprint: Some(curseforge_fingerprint(bytes)),
        }
    }

    pub async fn from_path(path: impl AsRef<Path>) -> LauncherResult<Self> {
        let bytes = polyio::read(path).await?;
        Ok(Self::from_bytes(&bytes))
    }

    pub fn from_sha1(sha1: impl AsRef<str>) -> Self {
        Self {
            sha1: normalize_hash(sha1.as_ref()),
            cf_fingerprint: None,
        }
    }

    pub fn with_curseforge_fingerprint(mut self, fingerprint: u32) -> Self {
        self.cf_fingerprint = Some(fingerprint);
        self
    }
}

pub fn curseforge_fingerprint(bytes: &[u8]) -> u32 {
    const MULTIPLIER: u32 = 1540483477;

    let normalized_len = bytes.iter().filter(|&&b| !is_cf_whitespace(b)).count();
    let mut hash = 1u32 ^ normalized_len as u32;
    let mut chunk = 0u32;
    let mut chunk_bits = 0u32;

    for &byte in bytes {
        if is_cf_whitespace(byte) {
            continue;
        }

        chunk |= u32::from(byte) << chunk_bits;
        chunk_bits += 8;

        if chunk_bits == 32 {
            let mixed = chunk.wrapping_mul(MULTIPLIER);
            let folded = (mixed ^ (mixed >> 24)).wrapping_mul(MULTIPLIER);
            hash = hash.wrapping_mul(MULTIPLIER) ^ folded;
            chunk = 0;
            chunk_bits = 0;
        }
    }

    if chunk_bits > 0 {
        hash = (hash ^ chunk).wrapping_mul(MULTIPLIER);
    }

    let mixed = (hash ^ (hash >> 13)).wrapping_mul(MULTIPLIER);
    mixed ^ (mixed >> 15)
}

#[inline]
fn is_cf_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}
