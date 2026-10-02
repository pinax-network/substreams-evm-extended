#![cfg(not(target_arch = "wasm32"))]
//! Host checks that the legacy `evm-balances` patch package stays 1:1 with the
//! deployed substreams-evm `evm-clickhouse-balances-v0.3.4.spkg`.

pub mod cli;
pub mod compare;
pub mod corpus;
pub mod spkg;
pub mod wasm;

#[cfg(test)]
mod tests;
