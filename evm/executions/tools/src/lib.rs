#![cfg(not(target_arch = "wasm32"))]
//! Host-only tools for `evm/executions`. Nothing here is linked into the map.
pub mod cli;
pub mod replay;

#[cfg(test)]
mod tests;
