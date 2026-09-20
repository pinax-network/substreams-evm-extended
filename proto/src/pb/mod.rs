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
