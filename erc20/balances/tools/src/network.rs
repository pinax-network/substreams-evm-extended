//! Host verification identity. A network label is configuration, not evidence:
//! only a matching `eth_chainId` response supplies a verified report chain ID.
use crate::rpc::{quantity, HttpRpc, Rpc};
use anyhow::{ensure, Result};
use clap::Args;
use serde_json::{json, Value};

#[derive(Args, Clone, Debug, Default)]
pub struct VerificationNetwork {
    /// Substreams network name. Supply together with --expected-chain-id;
    /// omitting both retains the historical BSC / 56 configuration.
    #[arg(long, requires = "expected_chain_id")]
    pub network: Option<String>,
    /// Independently expected eth_chainId, as a positive decimal integer.
    #[arg(long, requires = "network")]
    pub expected_chain_id: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Network {
    pub name: String,
    pub chain_id: u64,
}

impl VerificationNetwork {
    pub fn selected(&self) -> Result<Network> {
        let (name, chain_id) = match (&self.network, self.expected_chain_id) {
            (None, None) => ("bsc", 56),
            (Some(name), Some(chain_id)) => (name.as_str(), chain_id),
            _ => anyhow::bail!("supply both --network and --expected-chain-id"),
        };
        ensure!(
            !name.is_empty() && name.len() <= 64 && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_'),
            "network must be a short lowercase name, not an endpoint URL"
        );
        ensure!(chain_id > 0, "expected chain ID must be positive");
        let known = match name {
            "bsc" => Some(56),
            "mainnet" | "ethereum" => Some(1),
            "base" => Some(8453),
            "hyperevm" => Some(999),
            _ => None,
        };
        ensure!(known.is_none_or(|id| id == chain_id), "network name and expected chain ID disagree");
        Ok(Network { name: name.into(), chain_id })
    }

    /// Connect and verify before any header, runtime, balance or stream work.
    pub fn connect(&self, report: &mut Value) -> Result<HttpRpc> {
        let network = self.selected()?;
        network.record_configuration(report);
        let rpc = HttpRpc::from_env(&network)?;
        network.verify_rpc(&rpc, report)?;
        Ok(rpc)
    }
}

impl Network {
    pub fn is_bsc(&self) -> bool {
        self.name == "bsc" && self.chain_id == 56
    }

    pub fn record_configuration(&self, report: &mut Value) {
        report["network"] = json!(self.name);
        report["expected_chain_id"] = json!(self.chain_id);
        report["network_binding"] = json!("configured_only");
        // An offline run or failed check must never claim verified provenance.
        report.as_object_mut().unwrap().remove("chain_id");
        report.as_object_mut().unwrap().remove("rpc_chain_id");
    }

    pub fn verify_rpc(&self, rpc: &dyn Rpc, report: &mut Value) -> Result<()> {
        self.record_configuration(report);
        let actual = quantity(&rpc.call("eth_chainId", json!([]))?)?;
        ensure!(actual <= u64::MAX.into(), "RPC chain ID exceeds supported range");
        let actual = actual.low_u64();
        report["rpc_chain_id"] = json!(actual);
        ensure!(actual == self.chain_id, "RPC chain ID mismatch: expected {}, received {actual}", self.chain_id);
        report["chain_id"] = json!(actual);
        report["network_binding"] = json!("rpc_chain_id_verified");
        Ok(())
    }

    /// Older reports may omit identity. Their block hashes must still be
    /// checked against the selected RPC; any recorded identity must agree.
    pub fn check_report(&self, report: &Value) -> Result<()> {
        for field in ["chain_id", "expected_chain_id", "rpc_chain_id"] {
            if let Some(id) = report.get(field) {
                ensure!(crate::data::number(id)? == self.chain_id, "input report chain differs from configured chain");
            }
        }
        if let Some(name) = report.get("network") {
            ensure!(name == &self.name, "input report network differs from configured network");
        }
        Ok(())
    }

    /// Rankings have always recorded a chain ID, including historical BSC
    /// reports. Do not weaken that binding when accepting other networks.
    pub fn check_ranking(&self, report: &Value) -> Result<()> {
        ensure!(report["status"] == "ranked", "completed ranking required");
        ensure!(report.get("chain_id").is_some(), "ranking has no verified chain ID");
        self.check_report(report)
    }

    pub fn check_source_chain(&self, source: &Value) -> Result<()> {
        ensure!(
            crate::data::number(&source["chainId"])? == self.chain_id,
            "source chain differs from configured chain"
        );
        Ok(())
    }

    /// Credentials belong in environment variables, never in CLI endpoints
    /// which external capture programs may print in their logs.
    pub fn endpoint(&self, configured: Option<&str>, firehose: bool) -> Result<String> {
        let default = if firehose {
            "bsc.firehose.pinax.network:443"
        } else {
            "bsc.substreams.pinax.network:443"
        };
        let endpoint = match configured {
            Some(value) => value,
            None if self.is_bsc() => default,
            None => anyhow::bail!("--endpoint is required outside the BSC default configuration"),
        };
        let authority = endpoint
            .strip_prefix("https://")
            .or_else(|| endpoint.strip_prefix("http://"))
            .unwrap_or(endpoint);
        ensure!(
            !authority.is_empty() && !authority.bytes().any(|b| b.is_ascii_whitespace() || b"/@?#".contains(&b)),
            "endpoint must contain only a host and optional port; use environment variables for credentials"
        );
        Ok(endpoint.into())
    }

    pub fn rpc_url(&self, configured: Option<String>) -> Result<String> {
        match configured {
            Some(url) => {
                ensure!(!url.trim().is_empty(), "RPC_URL must not be empty");
                Ok(url)
            }
            None if self.is_bsc() => Ok("https://bsc.rpc.pinax.network".into()),
            None => Err(anyhow::anyhow!("RPC_URL is required outside the BSC default configuration")),
        }
    }
}

/// An environment variable containing non-Unicode data is invalid, not absent.
pub(crate) fn rpc_url_from_env(network: &Network) -> Result<String> {
    let configured = match std::env::var("RPC_URL") {
        Ok(value) => Some(value),
        Err(std::env::VarError::NotPresent) => None,
        Err(_) => anyhow::bail!("RPC_URL must be valid text"),
    };
    network.rpc_url(configured)
}

/// Preserve the legacy streaming-key fallback only at the exact HTTPS BSC
/// Pinax RPC origin. New providers must explicitly opt in with RPC_API_KEY.
pub(crate) fn rpc_key(url: &str, explicit: Option<String>, streaming: Option<String>) -> Option<String> {
    explicit.or_else(|| {
        matches!(
            url,
            "https://bsc.rpc.pinax.network" | "https://bsc.rpc.pinax.network/" | "https://bsc.rpc.pinax.network:443" | "https://bsc.rpc.pinax.network:443/"
        )
        .then_some(streaming)
        .flatten()
    })
}

#[cfg(test)]
mod tests;
