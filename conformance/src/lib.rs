#![cfg(not(target_arch = "wasm32"))]
//! Exact integer reference models for balances whose observable value depends
//! on shared protocol state and a block clock (issue #16). Every model is a
//! pure function of extracted inputs; missing input is an explicit error,
//! never zero. Rounding follows the pinned source of each epoch.

pub mod aave;
pub mod comet;
pub mod compound_v2;

/// Why a reference evaluation cannot produce a number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unknown {
    /// A required input was not supplied (uninitialized or not carried).
    MissingInput(&'static str),
    /// Inputs violate the model's domain (clock before last update,
    /// overflow of a bounded field, division by zero).
    Invalid(&'static str),
}
pub type Result<T> = std::result::Result<T, Unknown>;
