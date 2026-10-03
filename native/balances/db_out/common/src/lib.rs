//! The subset of substreams-evm's `common` crate that the native `db_out` reaches.
//!
//! Lines below the imports are copied verbatim from
//! `common/src/lib.rs` at pinax-network/substreams-evm@cb8607f59a37aa9daf85547162b8871d12568282
//! (lines 11–81, the commit that built `evm-balances-v0.3.4.spkg`), so address
//! encoding stays byte-for-byte identical to the legacy package.
#![allow(clippy::ptr_arg, clippy::len_zero, clippy::needless_borrows_for_generic_args)]
use sha2::{Digest, Sha256};
use substreams::Hex;

const TRON_VERSION_BYTE: u8 = 0x41; // 'T' addresses on Tron

/// Compute the 4-byte checksum for Base58Check (double SHA-256, first 4 bytes).
fn checksum4(data: &[u8]) -> [u8; 4] {
    let h1 = Sha256::digest(data);
    let h2 = Sha256::digest(&h1);
    let mut out = [0u8; 4];
    out.copy_from_slice(&h2[..4]);
    out
}

/// Convert a 20-byte payload (typically address body) into a Tron Base58Check string.
/// This prepends the Tron version byte (0x41) and appends the checksum.
pub fn tron_base58_from_20(bytes20: &[u8]) -> Result<String, &'static str> {
    if bytes20.len() != 20 {
        return Err("expected exactly 20 bytes");
    }
    let mut data = Vec::with_capacity(21 + 4);
    data.push(TRON_VERSION_BYTE);
    data.extend_from_slice(bytes20);
    let chk = checksum4(&data);
    data.extend_from_slice(&chk);
    Ok(bs58::encode(data).into_string())
}

/// Same as above, but accepts either:
/// - 20 bytes (address body) -> will prepend 0x41
/// - 21 bytes (already includes a leading version byte)
pub fn tron_base58_from_bytes(bytes: &[u8]) -> Result<String, &'static str> {
    match bytes.len() {
        20 => tron_base58_from_20(bytes),
        21 => {
            if bytes[0] != TRON_VERSION_BYTE {
                return Err("unexpected version byte; expected 0x41");
            }
            let mut data = bytes.to_vec();
            let chk = checksum4(&data);
            data.extend_from_slice(&chk);
            Ok(bs58::encode(data).into_string())
        }
        _ => Err("expected 20 or 21 bytes"),
    }
}

#[derive(PartialEq)]
pub enum Encoding {
    Hex,
    TronBase58,
}

pub fn handle_encoding_param(params: &String) -> Encoding {
    // Handle support both EVM & TVM address encoding
    if params.len() > 0 && params != "hex" && params != "tron_base58" {
        panic!("Invalid encoding parameter, supported: hex, tron_base58");
    }
    if params == "tron_base58" {
        return Encoding::TronBase58;
    }
    Encoding::Hex
}

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    format! {"0x{}", Hex::encode(bytes)}.to_string()
}

pub fn bytes_to_string(bytes: &[u8], encoding: &Encoding) -> String {
    if encoding == &Encoding::TronBase58 {
        return tron_base58_from_bytes(bytes).unwrap_or_default();
    }
    bytes_to_hex(bytes)
}
