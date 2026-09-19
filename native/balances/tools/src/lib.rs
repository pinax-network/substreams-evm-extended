#![cfg(not(target_arch = "wasm32"))]

pub mod cli;
pub mod replay;

#[cfg(test)]
mod tests;
