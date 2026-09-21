pub mod evm {
    pub mod balances {
        pub mod v1 {
            include!("evm.balances.v1.rs");
        }
    }
    pub mod balance_state {
        pub mod v1 {
            include!("evm.balance_state.v1.rs");
        }
    }
    pub mod executions {
        pub mod v1 {
            include!("evm.executions.v1.rs");
        }
    }
}
pub mod erc20 {
    pub mod events {
        pub mod v1 {
            include!("erc20.events.v1.rs");
        }
    }
}
pub mod aave {
    pub mod actions {
        pub mod v1 {
            include!("aave.actions.v1.rs");
        }
    }
}

pub mod uniswap {
    pub mod v2 {
        include!("uniswap.v2.rs");
    }
    pub mod v3 {
        include!("uniswap.v3.rs");
    }
}
pub mod dex {
    pub mod pool_state {
        pub mod v1 {
            include!("dex.pool_state.v1.rs");
        }
    }
}
