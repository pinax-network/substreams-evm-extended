//! Protobuf types for the packages of this repository, generated from
//! `proto/v1` by the `buf.build/anthropics/buffa` plugin (v0.9.2, see
//! `proto/buf.gen.yaml`); run `buf generate` in `proto/` to regenerate. This
//! file is written by hand and keeps the module paths of the earlier prost
//! code.

macro_rules! include_buffa {
    ($package:literal) => {
        include!(concat!($package, ".mod.rs"));
    };
}

pub mod evm {
    pub mod balances {
        pub mod v1 {
            include_buffa!("evm.balances.v1");
        }
    }
    pub mod balance_state {
        pub mod v1 {
            include_buffa!("evm.balance_state.v1");
        }
    }
    pub mod executions {
        pub mod v1 {
            include_buffa!("evm.executions.v1");
        }
    }
}
pub mod erc20 {
    pub mod events {
        pub mod v1 {
            include_buffa!("erc20.events.v1");
        }
    }
}
pub mod aave {
    pub mod actions {
        pub mod v1 {
            include_buffa!("aave.actions.v1");
        }
    }
}

pub mod uniswap {
    pub mod v2 {
        include_buffa!("uniswap.v2");
    }
    pub mod v3 {
        include_buffa!("uniswap.v3");
    }
}
pub mod dex {
    pub mod pool_state {
        pub mod v1 {
            include_buffa!("dex.pool_state.v1");
        }
    }
}
