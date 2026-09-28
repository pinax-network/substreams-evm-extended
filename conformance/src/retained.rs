//! Reference evaluation from retained, explicitly qualified protocol inputs.
//! This host-only bridge does not admit a runtime merely because a stream
//! names a familiar model. A caller supplies the exact reviewed stream,
//! model/dependency binding and input locations, with its evidence reference.
//! Results preserve their evaluation clock and every consumed original fact.
use crate::{aave, erc4626, lido, Result, Unknown};
use evm_retention::protocol::{same_model, Fact, GlobalKey, ProtocolLedger};
use evm_retention::Stream;
use num_bigint::{BigInt, BigUint};
use num_traits::ToPrimitive;
use proto::pb::evm::balance_state::v1 as pb;
use std::collections::BTreeMap;
use tiny_keccak::{Hasher, Keccak};

mod compound;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferenceModel {
    AaveAtoken(aave::Era),
    StaticAToken,
    SavingsDai,
    OzVirtualOffset,
    LidoV4,
    CompoundV2Cusdc2019,
    CompoundV2Ceth2019,
    CometUsdc,
}

/// Names intentionally distinguish ERC-20 amounts, share basis and projected
/// conversions. Log evidence has no holder-state evaluation metric here.
/// No result replaces Balance.amount.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    HolderBasis,
    AaveBalanceOf,
    StaticATokenConvertToAssets,
    StaticATokenRate,
    SavingsDaiConvertToAssets,
    OzConvertToAssets,
    LidoBalanceOf,
    CompoundV2ExchangeRateStored,
    CompoundV2StoredUnderlying,
    CompoundV2ProjectedUnderlying,
    CometPrincipal,
    CometStoredSupplyIndex,
    CometProjectedSupplyIndex,
    CometBalanceOf,
}

/// A principal can be negative; an observable amount, conversion or index
/// cannot. The metric and units remain part of every returned evaluation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetricValue {
    Unsigned(BigUint),
    SignedPrincipal(BigInt),
}
impl std::fmt::Display for MetricValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsigned(value) => value.fmt(f),
            Self::SignedPrincipal(value) => value.fmt(f),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetricUnits {
    /// Raw storage basis, without inferred display decimals.
    RawBasis(pb::BasisKind),
    /// Smallest units of the model's balance asset; empty contract is native.
    Asset {
        contract: Vec<u8>,
        decimals: u32,
    },
    /// Raw underlying units per raw share, multiplied by this scale.
    ConversionRate {
        scale: &'static str,
    },
    Index {
        scale: &'static str,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputBinding {
    pub field: pb::StateField,
    pub key: Vec<u8>,
    pub observation: pb::Observation,
    pub storage_contract: Vec<u8>,
    pub storage_slot: Vec<u8>,
    pub bit_offset: u32,
    pub bit_width: u32,
}

/// The supported models keep holder basis in one address-keyed mapping.
/// A different storage shape needs a separately reviewed binding variant.
#[derive(Clone, Debug)]
pub struct HolderBinding {
    pub storage_contract: Vec<u8>,
    pub mapping_slot: Vec<u8>,
}

pub fn holder_storage_slot(holder: &[u8], mapping_slot: &[u8]) -> Result<Vec<u8>> {
    if holder.len() != 20 || mapping_slot.len() != 32 {
        return Err(Unknown::Invalid("holder mapping identity has invalid width"));
    }
    let mut encoded = [0; 64];
    encoded[12..32].copy_from_slice(holder);
    encoded[32..].copy_from_slice(mapping_slot);
    let mut result = [0; 32];
    let mut hasher = Keccak::v256();
    hasher.update(&encoded);
    hasher.finalize(&mut result);
    Ok(result.to_vec())
}

/// The independently reviewed declaration expected from this exact stream.
/// Public fields make the qualification auditable; every evaluation validates
/// them afresh. An empty evidence reference or missing binding is Unknown.
#[derive(Clone, Debug)]
pub struct QualifiedModel {
    pub stream: Stream,
    pub epoch: pb::ModelEpoch,
    pub dependencies: Vec<pb::Dependency>,
    pub inputs: Vec<InputBinding>,
    pub holder_binding: HolderBinding,
    pub model: ReferenceModel,
    pub evidence: String,
    pub runtime: Option<RuntimeQualification>,
}

/// Explicit external attestation, not a fact inferred from `source_pin` or
/// fetched by this library. It binds the runtime/dependency set at the exact
/// model origin (a BOUND block or verified checkpoint). The caller owns its
/// independent evidence and source/runtime equality. Empty hashes in emitted
/// Aave declarations therefore do not silently qualify a runtime.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeQualification {
    pub at: pb::BlockClock,
    pub market_code_hash: Vec<u8>,
    pub implementation_code_hash: Vec<u8>,
    pub dependency_code_hashes: BTreeMap<Vec<u8>, Vec<u8>>,
    pub evidence: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Evaluation {
    pub metric: Metric,
    pub value: MetricValue,
    pub units: MetricUnits,
    pub clock: pb::BlockClock,
    pub model: pb::ModelEpoch,
    pub qualification_evidence: String,
    pub runtime_qualification: RuntimeQualification,
    pub holder: Option<Fact<pb::HolderBasis>>,
    pub globals: Vec<Fact<pb::GlobalState>>,
}

const RAY: &str = "1000000000000000000000000000";
const AAVE_SOURCE: &str = "aave-dao/aave-v3-origin@8305565ae342f1773c42cd2e4593f175fe5968a0";
const STATA_SOURCE: &str = "bgd-labs/static-a-token-v3@101f5d977889254ca2d2711b9582b45f832d10a0";
const SDAI_SOURCE: &str = "sky-ecosystem/sdai@665879762f8b5df5d234463f45d1d6a49bd4fbeb";
const LIDO_SOURCE: &str = "lidofinance/core@2da0f48f1a2a103a394dcf8760810fe9165697fb";
const OZ_SOURCE: &str = "OpenZeppelin/openzeppelin-contracts@932fddf69a699a9a80fd2396fd1a2ab91cdda123";
const OZ_UPGRADEABLE_SOURCE: &str = "OpenZeppelin/openzeppelin-contracts-upgradeable@625fb3c2b2696f1747ba2e72d1e1113066e6c177";

fn pinned(pin: &str, expected: &str) -> bool {
    pin.strip_prefix(expected).is_some_and(|suffix| suffix.is_empty() || suffix.starts_with(' '))
}
fn number(value: &str) -> Result<BigUint> {
    value.parse().map_err(|_| Unknown::Invalid("unsigned exact input required"))
}

impl QualifiedModel {
    fn validate<'a>(&self, ledger: &'a ProtocolLedger) -> Result<&'a pb::BlockClock> {
        if self.evidence.trim().is_empty() {
            return Err(Unknown::MissingInput("model qualification evidence"));
        }
        if ledger.stream() != Some(&self.stream) {
            return Err(Unknown::Invalid("qualified stream identity mismatch"));
        }
        let current = ledger.model(&self.epoch.market).map_err(|_| Unknown::MissingInput("active model binding"))?;
        if !same_model(&current.row, &self.epoch) {
            return Err(Unknown::Invalid("qualified model epoch mismatch"));
        }
        let runtime = self.runtime.as_ref().ok_or(Unknown::MissingInput("external runtime qualification"))?;
        if runtime.evidence.trim().is_empty() {
            return Err(Unknown::MissingInput("external runtime evidence"));
        }
        if !same_origin_header(&runtime.at, &current.observed_at)
            || runtime.market_code_hash.len() != 32
            || (!self.epoch.market_code_hash.is_empty() && self.epoch.market_code_hash != runtime.market_code_hash)
            || (self.epoch.implementation.is_empty() && !runtime.implementation_code_hash.is_empty())
            || (!self.epoch.implementation.is_empty() && runtime.implementation_code_hash.len() != 32)
            || (!self.epoch.implementation_code_hash.is_empty() && self.epoch.implementation_code_hash != runtime.implementation_code_hash)
        {
            return Err(Unknown::Invalid("runtime attestation differs from exact model origin or declared hashes"));
        }
        for dependency in &self.dependencies {
            let hash = runtime
                .dependency_code_hashes
                .get(&dependency.contract)
                .ok_or(Unknown::MissingInput("external dependency runtime qualification"))?;
            if hash.len() != 32 || (!dependency.code_hash.is_empty() && dependency.code_hash != *hash) {
                return Err(Unknown::Invalid("dependency runtime attestation differs from declared hash"));
            }
            if (dependency.contract == self.epoch.market && *hash != runtime.market_code_hash)
                || (dependency.contract == self.epoch.implementation && *hash != runtime.implementation_code_hash)
            {
                return Err(Unknown::Invalid("one runtime address has conflicting attestations"));
            }
        }
        let expected_deps = normalized_dependencies(&self.dependencies);
        let actual_deps = normalized_dependencies(
            ledger
                .dependencies(&self.epoch.market)
                .map_err(|_| Unknown::MissingInput("model dependencies"))?,
        );
        if expected_deps != actual_deps {
            return Err(Unknown::Invalid("qualified dependency mismatch"));
        }
        if self.is_compound() {
            self.validate_compound()?;
            return ledger.clock().ok_or(Unknown::MissingInput("canonical evaluation clock"));
        }
        let (family, id, source, scale, basis, rounding) = match self.model {
            ReferenceModel::AaveAtoken(era) => (
                pb::ModelFamily::AaveV3Atoken,
                match era {
                    aave::Era::Floor => "aave-v3/atoken/scaled-floor",
                    aave::Era::HalfUp => "aave-v3/atoken/scaled-half-up",
                },
                AAVE_SOURCE,
                RAY,
                pb::BasisKind::ScaledBalance,
                match era {
                    aave::Era::Floor => pb::Rounding::Floor,
                    aave::Era::HalfUp => pb::Rounding::HalfUp,
                },
            ),
            ReferenceModel::StaticAToken => (
                pb::ModelFamily::Erc4626Vault,
                "erc4626/aave-static-atoken-lm/ray-mul-round-down",
                STATA_SOURCE,
                RAY,
                pb::BasisKind::Shares,
                pb::Rounding::Floor,
            ),
            ReferenceModel::SavingsDai => (
                pb::ModelFamily::Erc4626Vault,
                "erc4626/maker-savings-dai/rpow-chi",
                SDAI_SOURCE,
                RAY,
                pb::BasisKind::Shares,
                pb::Rounding::Floor,
            ),
            ReferenceModel::OzVirtualOffset => (
                pb::ModelFamily::Erc4626Vault,
                "erc4626/openzeppelin-v5/virtual-offset",
                OZ_SOURCE,
                "",
                pb::BasisKind::Shares,
                pb::Rounding::Floor,
            ),
            ReferenceModel::LidoV4 => (
                pb::ModelFamily::LidoSteth,
                "lido/steth/v4/internal-share-rate",
                LIDO_SOURCE,
                "1",
                pb::BasisKind::Shares,
                pb::Rounding::Floor,
            ),
            ReferenceModel::CompoundV2Cusdc2019 | ReferenceModel::CompoundV2Ceth2019 | ReferenceModel::CometUsdc => unreachable!(),
        };
        if self.epoch.family != family as i32
            || self.epoch.model_id != id
            || self.epoch.basis_kind != basis as i32
            || self.epoch.basis_scale != scale
            || self.epoch.balance_rounding != rounding as i32
            || self.epoch.basis_signed
            || !(pinned(&self.epoch.source_pin, source)
                || (self.model == ReferenceModel::OzVirtualOffset && pinned(&self.epoch.source_pin, OZ_UPGRADEABLE_SOURCE)))
        {
            return Err(Unknown::Invalid("unsupported formula, source, basis scale or rounding"));
        }
        let revision = self.epoch.implementation_revision.parse::<u32>().ok();
        let revision_matches = match self.model {
            ReferenceModel::AaveAtoken(aave::Era::Floor) => revision.is_some_and(|r| r >= 4),
            ReferenceModel::AaveAtoken(aave::Era::HalfUp) => revision.is_some_and(|r| (1..=3).contains(&r)),
            ReferenceModel::StaticAToken => revision == Some(2),
            ReferenceModel::LidoV4 => revision == Some(4),
            ReferenceModel::SavingsDai | ReferenceModel::OzVirtualOffset => true,
            ReferenceModel::CompoundV2Cusdc2019 | ReferenceModel::CompoundV2Ceth2019 | ReferenceModel::CometUsdc => unreachable!(),
        };
        if !revision_matches {
            return Err(Unknown::Invalid("implementation revision does not match the arithmetic era"));
        }
        for (index, binding) in self.inputs.iter().enumerate() {
            if self.inputs[..index].iter().any(|other| other.field == binding.field) {
                return Err(Unknown::Invalid("ambiguous qualified input field"));
            }
            if binding.storage_contract.len() != 20
                || (binding.storage_contract != self.epoch.market
                    && binding.storage_contract != self.epoch.implementation
                    && !self.dependencies.iter().any(|d| d.contract == binding.storage_contract))
            {
                return Err(Unknown::MissingInput("qualified input contract dependency"));
            }
        }
        ledger.clock().ok_or(Unknown::MissingInput("canonical evaluation clock"))
    }

    pub fn evaluate(&self, ledger: &ProtocolLedger, holder: &[u8], metric: Metric) -> Result<Evaluation> {
        let clock = self.validate(ledger)?.clone();
        if self.is_compound() {
            return self.evaluate_compound(ledger, holder, metric, clock);
        }
        let mut result = Evaluation {
            metric,
            value: MetricValue::Unsigned(BigUint::default()),
            units: match metric {
                Metric::HolderBasis => MetricUnits::RawBasis(pb::BasisKind::try_from(self.epoch.basis_kind).map_err(|_| Unknown::Invalid("basis kind"))?),
                Metric::StaticATokenRate => MetricUnits::ConversionRate { scale: RAY },
                _ => MetricUnits::Asset {
                    contract: self.epoch.balance_asset.clone(),
                    decimals: self.epoch.balance_decimals,
                },
            },
            clock: clock.clone(),
            model: self.epoch.clone(),
            qualification_evidence: self.evidence.clone(),
            runtime_qualification: self.runtime.as_ref().expect("runtime qualification validated above").clone(),
            holder: None,
            globals: Vec::new(),
        };
        let basis = if metric == Metric::StaticATokenRate {
            None
        } else {
            let fact = ledger
                .holder(&self.epoch.market, holder)
                .map_err(|_| Unknown::MissingInput("initialized holder basis"))?;
            if fact.row.basis_kind != self.epoch.basis_kind
                || fact.row.signed
                || fact.row.bit_offset != self.epoch.basis_bit_offset
                || fact.row.bit_width != self.epoch.basis_bit_width
            {
                return Err(Unknown::Invalid("holder basis layout differs from model"));
            }
            if self.holder_binding.storage_contract.len() != 20
                || self.holder_binding.storage_contract != self.epoch.market
                || fact.row.storage_contract != self.holder_binding.storage_contract
                || fact.row.storage_slot != holder_storage_slot(holder, &self.holder_binding.mapping_slot)?
            {
                return Err(Unknown::Invalid("holder storage provenance differs from qualification"));
            }
            let value = number(&fact.row.value)?;
            if value.bits() > u64::from(self.epoch.basis_bit_width) {
                return Err(Unknown::Invalid("holder basis exceeds model bit width"));
            }
            result.holder = Some(fact.clone());
            Some(value)
        };
        let mut read = |field, scale, observation| self.read(ledger, field, scale, observation, &mut result.globals);
        let stored = pb::Observation::ObservedWrite;
        let shares = || basis.as_ref().ok_or(Unknown::MissingInput("holder basis"));
        result.value = MetricValue::Unsigned(match (metric, self.model) {
            (Metric::HolderBasis, _) => shares()?.clone(),
            (Metric::AaveBalanceOf, ReferenceModel::AaveAtoken(era)) => aave::balance_of(shares()?, &read_reserve(&mut read)?, clock.timestamp, era)?,
            (Metric::StaticATokenConvertToAssets | Metric::StaticATokenRate, ReferenceModel::StaticAToken) => {
                let model = erc4626::StataTokenLm {
                    reserve: read_reserve(&mut read)?,
                    reserve_active_and_unpaused: None,
                };
                if metric == Metric::StaticATokenRate {
                    model.rate(clock.timestamp)?
                } else {
                    model.convert_to_assets(shares()?, clock.timestamp)?
                }
            }
            (Metric::SavingsDaiConvertToAssets, ReferenceModel::SavingsDai) => {
                let model = erc4626::SavingsDai {
                    chi: read(pb::StateField::MakerPotChi, RAY, stored)?,
                    dsr: read(pb::StateField::MakerPotDsr, RAY, stored)?,
                    rho: read(pb::StateField::MakerPotRho, "1", stored)?,
                };
                model.convert_to_assets(shares()?, clock.timestamp)?
            }
            (Metric::OzConvertToAssets, ReferenceModel::OzVirtualOffset) => {
                let model = erc4626::OzVirtualOffset {
                    total_assets: read(pb::StateField::Erc4626TotalAssets, "1", stored)?,
                    total_supply: read(pb::StateField::Erc4626TotalSupply, "1", stored)?,
                    decimals_offset: read(pb::StateField::Erc4626DecimalsOffset, "1", pb::Observation::QualifiedConstant)?
                        .to_u8()
                        .ok_or(Unknown::Invalid("decimals offset exceeds u8"))?,
                };
                model.convert_to_assets(shares()?)?
            }
            (Metric::LidoBalanceOf, ReferenceModel::LidoV4) => {
                let model = lido::Pool {
                    total_shares: read(pb::StateField::LidoTotalShares, "1", stored)?,
                    external_shares: read(pb::StateField::LidoExternalShares, "1", stored)?,
                    buffered_ether: read(pb::StateField::LidoBufferedEther, "1", stored)?,
                    deposited_post_report: read(pb::StateField::LidoDepositedPostReport, "1", stored)?,
                    cl_validators_balance: read(pb::StateField::LidoClValidatorsBalance, "1", stored)?,
                    cl_pending_balance: read(pb::StateField::LidoClPendingBalance, "1", stored)?,
                };
                lido::balance_of(Some(shares()?), &model)?
            }
            _ => return Err(Unknown::Invalid("metric does not belong to the qualified model")),
        });
        Ok(result)
    }

    fn read(
        &self,
        ledger: &ProtocolLedger,
        field: pb::StateField,
        scale: &str,
        observation: pb::Observation,
        consumed: &mut Vec<Fact<pb::GlobalState>>,
    ) -> Result<BigUint> {
        let binding = self
            .inputs
            .iter()
            .find(|input| input.field == field)
            .ok_or(Unknown::MissingInput("qualified global input location"))?;
        if binding.observation != observation {
            return Err(Unknown::Invalid("observation is not the model's input kind"));
        }
        let fact = ledger
            .global(&GlobalKey::new(&self.epoch.market, field, &binding.key, observation))
            .map_err(|_| Unknown::MissingInput("initialized global input"))?;
        let row = &fact.row;
        if row.scale != scale
            || row.signed
            || row.storage_contract != binding.storage_contract
            || row.storage_slot != binding.storage_slot
            || row.bit_offset != binding.bit_offset
            || row.bit_width != binding.bit_width
        {
            return Err(Unknown::Invalid("global input scale or provenance differs from qualification"));
        }
        let runtime = self.runtime.as_ref().ok_or(Unknown::MissingInput("external runtime qualification"))?;
        if observation == pb::Observation::QualifiedConstant
            && !(self.epoch.implementation == row.storage_contract && runtime.implementation_code_hash.len() == 32
                || self.epoch.market == row.storage_contract && runtime.market_code_hash.len() == 32
                || runtime.dependency_code_hashes.get(&row.storage_contract).is_some_and(|hash| hash.len() == 32))
        {
            return Err(Unknown::MissingInput("constant runtime code-hash binding"));
        }
        let value = number(&row.value)?;
        if row.bit_width > 0 && value.bits() > u64::from(row.bit_width) {
            return Err(Unknown::Invalid("global input exceeds its bit width"));
        }
        consumed.push(fact.clone());
        Ok(value)
    }
}

fn normalized_dependencies(rows: &[pb::Dependency]) -> Vec<pb::Dependency> {
    let mut rows = rows.to_vec();
    for row in &mut rows {
        row.kind = pb::EpochEventKind::Bound as i32;
    }
    rows.sort_by(|a, b| (&a.market, a.epoch, a.role, &a.contract, &a.parent).cmp(&(&b.market, b.epoch, b.role, &b.contract, &b.parent)));
    rows
}

fn same_origin_header(a: &pb::BlockClock, b: &pb::BlockClock) -> bool {
    // A checkpoint may initialize only a subset of inputs at the same
    // independently attested header. Table counts prove atomic delivery of
    // that declared subset; they are not runtime/header identity fields.
    fn identity(mut clock: pb::BlockClock) -> pb::BlockClock {
        clock.epoch_count = 0;
        clock.dependency_count = 0;
        clock.alias_count = 0;
        clock.holder_basis_count = 0;
        clock.global_state_count = 0;
        clock
    }
    identity(a.clone()) == identity(b.clone())
}

fn read_reserve(read: &mut impl FnMut(pb::StateField, &'static str, pb::Observation) -> Result<BigUint>) -> Result<aave::Reserve> {
    let stored = pb::Observation::ObservedWrite;
    Ok(aave::Reserve {
        liquidity_index: read(pb::StateField::AaveLiquidityIndex, RAY, stored)?,
        current_liquidity_rate: read(pb::StateField::AaveCurrentLiquidityRate, RAY, stored)?,
        last_update_timestamp: read(pb::StateField::AaveLastUpdateTimestamp, "1", stored)?
            .to_u64()
            .ok_or(Unknown::Invalid("reserve timestamp exceeds u64"))?,
    })
}
