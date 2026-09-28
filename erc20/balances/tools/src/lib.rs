#![cfg(not(target_arch = "wasm32"))]

pub mod artx_role;
pub mod audit;
pub mod bas_role;
pub mod btr_proof;
pub mod burnmint_role;
pub mod capture;
pub mod cli;
pub mod comparison;
pub mod coverage;
pub mod data;
pub mod fhe_b2_roles;
pub mod inspect;
pub mod inspect_ranked;
pub mod lbp_rewards;
pub mod network;
pub mod oft_roles;
pub mod og_model;
pub mod package;
pub mod point_bedrock_roles;
pub mod probe;
pub mod ptoken_proof;
pub mod ptoken_role;
pub mod ranking;
pub mod recheck;
pub mod reflection;
pub mod refusal_scan;
pub mod role_operations;
pub mod rpc;
pub mod runtime_status;
pub mod survey;
pub mod tagger_role;
pub mod trace_context;
pub mod ybc_rewards;

#[cfg(test)]
mod tests;
