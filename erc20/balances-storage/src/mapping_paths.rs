//! Exact, caller-qualified paths to non-balance mapping fields.
use crate::{hash, hex_bytes, require, subtract_offset};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use substreams::errors::Error;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MappingPath {
    pub root: String,
    /// Mapping keys in Solidity declaration order, from outermost to innermost.
    pub key_types: Vec<String>,
    /// Offset in the terminal record, never applied to an intermediate mapping.
    #[serde(default)]
    pub offset: u8,
    #[serde(default = "one_word")]
    pub words: u8,
}

fn one_word() -> u8 {
    1
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MappingKeyType {
    Address,
    Bytes32,
    Uint(u16),
}

impl MappingKeyType {
    fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "address" => Ok(Self::Address),
            "bytes32" => Ok(Self::Bytes32),
            _ => {
                let bits = value.strip_prefix("uint").and_then(|s| s.parse::<u16>().ok());
                let bits = bits.filter(|bits| (8..=256).contains(bits) && bits % 8 == 0 && value == format!("uint{bits}"));
                bits.map(Self::Uint)
                    .ok_or_else(|| Error::msg("mapping path key must be address, bytes32, or canonical uint8..uint256"))
            }
        }
    }

    fn canonical(&self, key: &[u8]) -> bool {
        let leading = match self {
            Self::Address => 12,
            Self::Bytes32 => 0,
            Self::Uint(bits) => 32 - usize::from(*bits / 8),
        };
        key.len() == 32 && key[..leading].iter().all(|byte| *byte == 0)
    }
}

#[derive(Clone, Debug)]
pub struct VerifiedMappingPath {
    pub(crate) root: [u8; 32],
    pub(crate) key_types: Vec<MappingKeyType>,
    pub(crate) offset: u8,
    pub(crate) words: u8,
}

pub(crate) fn parse(paths: Vec<MappingPath>, reserved: &BTreeSet<[u8; 32]>) -> Result<Vec<VerifiedMappingPath>, Error> {
    let mut parsed: Vec<VerifiedMappingPath> = Vec::new();
    for path in paths {
        require(path.root.starts_with("0x"), "mapping path root must be 0x-prefixed hex")?;
        let root = hex_bytes(&path.root)?;
        require(root.len() == 32, "mapping path root must be 32 bytes")?;
        let root: [u8; 32] = root.try_into().unwrap();
        require(!reserved.contains(&root), "mapping path root overlaps another configured field")?;
        require((1..=8).contains(&path.key_types.len()), "mapping path depth must be 1..=8")?;
        require((1..=32).contains(&path.words), "mapping path width must be 1..=32 words")?;
        let end = u16::from(path.offset) + u16::from(path.words);
        require(end <= 256, "mapping path field range exceeds offset 255")?;
        let key_types = path.key_types.iter().map(|key| MappingKeyType::parse(key)).collect::<Result<Vec<_>, _>>()?;
        for prior in parsed.iter().filter(|prior| prior.root == root) {
            require(
                prior.key_types.iter().zip(&key_types).all(|(a, b)| a == b),
                "mapping paths disagree on a shared key prefix",
            )?;
            if prior.key_types.len() == key_types.len() {
                let prior_end = u16::from(prior.offset) + u16::from(prior.words);
                require(
                    u16::from(path.offset) >= prior_end || u16::from(prior.offset) >= end,
                    "mapping path field ranges overlap",
                )?;
            } else {
                let shorter_offset = if prior.key_types.len() < key_types.len() { prior.offset } else { path.offset };
                require(shorter_offset != 0, "mapping anchor cannot also be a terminal scalar field")?;
            }
        }
        parsed.push(VerifiedMappingPath {
            root,
            key_types,
            offset: path.offset,
            words: path.words,
        });
    }
    Ok(parsed)
}

pub(crate) fn matches(key: [u8; 32], preimages: &BTreeMap<[u8; 32], Vec<u8>>, paths: &[VerifiedMappingPath]) -> bool {
    paths.iter().any(|path| {
        let end = u16::from(path.offset) + u16::from(path.words);
        (u16::from(path.offset)..end).any(|offset| {
            let mut current = subtract_offset(key, offset as u8);
            for key_type in path.key_types.iter().rev() {
                let Some(preimage) = preimages.get(&current).filter(|preimage| preimage.len() == 64) else {
                    return false;
                };
                // Callers already verify collected preimages. Keep the matcher
                // independently strict so an unverified map cannot relax it.
                if hash(preimage) != current || !key_type.canonical(&preimage[..32]) {
                    return false;
                }
                current.copy_from_slice(&preimage[32..]);
            }
            current == path.root
        })
    })
}
