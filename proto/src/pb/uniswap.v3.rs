// @generated
// Pool-state types retained from the original schema; unrelated ABI projections omitted.
/// Complete block envelope for state reconstruction. Each pool's changes are in
/// canonical log order. This is not a standalone price or liquidity snapshot:
/// consumers need a verified initial state and uninterrupted block continuity.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BlockPoolChanges {
    #[prost(uint64, tag="1")]
    pub block_number: u64,
    #[prost(bytes="vec", tag="2")]
    pub block_hash: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes="vec", tag="3")]
    pub parent_hash: ::prost::alloc::vec::Vec<u8>,
    #[prost(int64, tag="4")]
    pub timestamp_seconds: i64,
    #[prost(int32, tag="5")]
    pub timestamp_nanos: i32,
    #[prost(message, repeated, tag="6")]
    pub pools: ::prost::alloc::vec::Vec<PoolChanges>,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PoolChanges {
    #[prost(bytes="vec", tag="1")]
    pub pool: ::prost::alloc::vec::Vec<u8>,
    #[prost(message, repeated, tag="2")]
    pub changes: ::prost::alloc::vec::Vec<PoolChange>,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PoolChange {
    #[prost(uint32, tag="1")]
    pub block_log_index: u32,
    #[prost(uint64, tag="2")]
    pub ordinal: u64,
    #[prost(oneof="pool_change::Change", tags="3, 4, 5, 6")]
    pub change: ::core::option::Option<pool_change::Change>,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PoolPriceState {
    /// uint160, unscaled Q64.96
    #[prost(string, tag="1")]
    pub sqrt_price_x96: ::prost::alloc::string::String,
    /// int24, including the zero-for-one boundary convention
    #[prost(int32, tag="2")]
    pub tick: i32,
    /// uint128; zero for Initialize
    #[prost(string, tag="3")]
    pub liquidity: ::prost::alloc::string::String,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PoolLiquidityChange {
    /// inclusive int24
    #[prost(int32, tag="1")]
    pub tick_lower: i32,
    /// exclusive int24
    #[prost(int32, tag="2")]
    pub tick_upper: i32,
    /// signed decimal: Mint positive, Burn negative
    #[prost(string, tag="3")]
    pub liquidity_delta: ::prost::alloc::string::String,
}

/// A recognized topic with an invalid shape/value. Consumers must invalidate this
/// pool rather than silently carry its previous state. No raw payload is copied.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InvalidPoolChange {
    #[prost(string, tag="1")]
    pub event_name: ::prost::alloc::string::String,
}

/// Nested message and enum types in `PoolChange`.
pub mod pool_change {
    #[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Change {
        #[prost(message, tag="3")]
        Initialize(super::PoolPriceState),
        #[prost(message, tag="4")]
        Swap(super::PoolPriceState),
        #[prost(message, tag="5")]
        Liquidity(super::PoolLiquidityChange),
        #[prost(message, tag="6")]
        Invalid(super::InvalidPoolChange),
    }
}
