//! Minimal `sf.substreams.v1.Package` reader for comparing packaged modules.

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use prost::Message;
use sha2::{Digest, Sha256};

/// The `sf.substreams.v1.Package` fields these checks read.
#[derive(Clone, PartialEq, Message)]
pub struct Package {
    #[prost(message, optional, tag = "6")]
    pub modules: Option<Modules>,
    #[prost(string, tag = "9")]
    pub network: String,
    #[prost(message, optional, tag = "10")]
    pub sink_config: Option<prost_types::Any>,
    #[prost(string, tag = "11")]
    pub sink_module: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct Modules {
    #[prost(message, repeated, tag = "1")]
    pub modules: Vec<Module>,
    #[prost(message, repeated, tag = "2")]
    pub binaries: Vec<Binary>,
}

#[derive(Clone, PartialEq, Message)]
pub struct Binary {
    #[prost(string, tag = "1")]
    pub r#type: String,
    #[prost(bytes = "vec", tag = "2")]
    pub content: Vec<u8>,
}

/// `sf.substreams.v1.Module`, including the `params` input that the SDK's
/// generated copy predates.
#[derive(Clone, PartialEq, Message)]
pub struct Module {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(uint32, tag = "4")]
    pub binary_index: u32,
    #[prost(string, tag = "5")]
    pub binary_entrypoint: String,
    #[prost(message, repeated, tag = "6")]
    pub inputs: Vec<Input>,
    #[prost(message, optional, tag = "7")]
    pub output: Option<Output>,
    #[prost(uint64, tag = "8")]
    pub initial_block: u64,
}

#[derive(Clone, PartialEq, Message)]
pub struct Input {
    #[prost(oneof = "InputKind", tags = "1, 2, 3, 4")]
    pub kind: Option<InputKind>,
}

/// Every input kind carries its identifying string in field 1: `Source.type`,
/// `Map.module_name`, `Store.module_name` and `Params.value`.
#[derive(Clone, PartialEq, prost::Oneof)]
pub enum InputKind {
    #[prost(message, tag = "1")]
    Source(Named),
    #[prost(message, tag = "2")]
    Map(Named),
    #[prost(message, tag = "3")]
    Store(Named),
    #[prost(message, tag = "4")]
    Params(Named),
}

#[derive(Clone, PartialEq, Message)]
pub struct Named {
    #[prost(string, tag = "1")]
    pub value: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct Output {
    #[prost(string, tag = "1")]
    pub r#type: String,
}

/// `sf.substreams.sink.sql.v1.Service`: what the legacy SQL sink applies.
#[derive(Clone, PartialEq, Message)]
pub struct SqlService {
    #[prost(string, tag = "1")]
    pub schema: String,
    #[prost(int32, tag = "7")]
    pub engine: i32,
}

impl Package {
    pub fn read(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        Package::decode(bytes.as_slice()).with_context(|| format!("decoding {}", path.display()))
    }

    pub fn modules(&self) -> &[Module] {
        self.modules.as_ref().map(|m| m.modules.as_slice()).unwrap_or_default()
    }

    pub fn module(&self, name: &str) -> Result<&Module> {
        self.modules()
            .iter()
            .find(|m| m.name == name)
            .ok_or_else(|| anyhow!("package has no module {name:?}"))
    }

    pub fn binary(&self, module: &Module) -> Result<&[u8]> {
        self.modules
            .as_ref()
            .and_then(|m| m.binaries.get(module.binary_index as usize))
            .map(|b| b.content.as_slice())
            .ok_or_else(|| anyhow!("module {:?} references a missing binary", module.name))
    }

    pub fn sql_service(&self) -> Result<SqlService> {
        let any = self.sink_config.as_ref().ok_or_else(|| anyhow!("package has no sink config"))?;
        SqlService::decode(any.value.as_slice()).context("decoding the SQL sink config")
    }
}

impl Module {
    /// Inputs as `kind:value`, e.g. `map:db:native_balances:map_events`.
    pub fn input_descriptions(&self) -> Vec<String> {
        self.inputs
            .iter()
            .map(|input| match &input.kind {
                Some(InputKind::Source(n)) => format!("source:{}", n.value),
                Some(InputKind::Map(n)) => format!("map:{}", n.value),
                Some(InputKind::Store(n)) => format!("store:{}", n.value),
                Some(InputKind::Params(n)) => format!("params:{}", n.value),
                None => "unknown".to_string(),
            })
            .collect()
    }

    pub fn output_type(&self) -> &str {
        self.output.as_ref().map(|o| o.r#type.as_str()).unwrap_or_default()
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
