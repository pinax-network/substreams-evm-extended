// @generated
// Pool-state types retained from the original schema; unrelated ABI projections omitted.
/// The final Sync observation for a pool in one complete block. This is raw
/// protocol state, not a verified token identity, a USD price or a TWAP.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PoolClose {
    #[prost(bytes="vec", tag="1")]
    pub pool: ::prost::alloc::vec::Vec<u8>,
    /// uint112; zero is preserved for empty liquidity
    #[prost(string, tag="2")]
    pub reserve0: ::prost::alloc::string::String,
    /// uint112
    #[prost(string, tag="3")]
    pub reserve1: ::prost::alloc::string::String,
    #[prost(uint32, tag="4")]
    pub block_log_index: u32,
    #[prost(uint64, tag="5")]
    pub ordinal: u64,
    /// At least one matching Sync in this block had an invalid ABI shape or value.
    /// Reserves are empty strings; consumers must invalidate cached pool state.
    #[prost(bool, tag="6")]
    pub invalid: bool,
}

/// Always emitted, including blocks without a matching Sync. Only pools updated
/// in this block are listed; absence is not a zero reserve or a fresh observation.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BlockPoolCloses {
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
    pub pools: ::prost::alloc::vec::Vec<PoolClose>,
}

