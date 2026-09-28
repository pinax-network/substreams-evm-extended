//! Bounded adapters for the committed cUSDC/cETH 2019 and USDC Comet
//! qualification profiles. Locations and source families are deliberately
//! narrow; runtime and initialized-state evidence remain caller attestations.
use super::*;
use crate::{comet, compound_v2 as v2};
use num_traits::Zero;

const EXP: &str = "1000000000000000000";
const INDEX: &str = "1000000000000000";
const CTOKEN_SOURCE: &str = "compound-finance/compound-protocol@f385d71983ae5c5799faae9b2dfea43e5cf75262";
const JUMP_SOURCE: &str = "compound-v2/legacy-jump-rate-model-v2 compound-finance/compound-protocol@4caf72a1f88335adc9cc06acf6f372241369ed01";
const WHITE_SOURCE: &str = "compound-v2/white-paper-2019 compound-finance/compound-protocol@f385d71983ae5c5799faae9b2dfea43e5cf75262";
const USDC_SOURCE: &str = "erc20/fiat-token-v2_2/balanceAndBlacklistStates-low255 circlefin/stablecoin-evm@v2.2.0 (405efc100c016ed1a437063b6274b4e24ea7b8b1)";
const COMET_SOURCE: &str = "compound-finance/comet@f766f51583c23acc33b2a7824654ef2029a96804";
const USDC: &str = "a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
const CUSDC: &str = "39aa39c021dfbae8fac545936693ac917d5e7563";
const CETH: &str = "4ddc2d193948926d02f9b1fe9e1daa0718270ed5";
const COMET: &str = "c3d688b66703497daa19211eedff47f25384cdc3";
const EIP1967: &str = "360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc";
const USDC_IMPL: &str = "7050c9e0f4ca769c69bd3a8ef740bc37934f8e2c036e5a723fd8ee048ed3f8c3";

fn bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .chunks_exact(2)
        .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
        .collect()
}
fn slot(n: u8) -> Vec<u8> {
    let mut value = vec![0; 32];
    value[31] = n;
    value
}
fn require(ok: bool, why: &'static str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Unknown::Invalid(why))
    }
}
fn uint64(value: BigUint) -> Result<u64> {
    value.to_u64().ok_or(Unknown::Invalid("input exceeds uint64"))
}

impl QualifiedModel {
    pub(super) fn is_compound(&self) -> bool {
        matches!(
            self.model,
            ReferenceModel::CompoundV2Cusdc2019 | ReferenceModel::CompoundV2Ceth2019 | ReferenceModel::CometUsdc
        )
    }

    fn dependency(&self, role: pb::DependencyRole, depth: u32, pin: &str) -> Result<&pb::Dependency> {
        let rows: Vec<_> = self.dependencies.iter().filter(|row| row.role == role as i32 && row.depth == depth).collect();
        require(
            rows.len() == 1 && pinned(&rows[0].source_pin, pin),
            "missing or unsupported Compound dependency",
        )?;
        Ok(rows[0])
    }

    fn pointer(&self, row: &pb::Dependency, owner: &[u8], key: &[u8], parent: &[u8]) -> Result<()> {
        let mut value = vec![0; 12];
        value.extend(&row.contract);
        require(
            row.binding == pb::BindingKind::StoragePointer as i32
                && row.pointer_contract == owner
                && row.pointer_slot == key
                && row.pointer_value == value
                && row.parent == parent,
            "Compound dependency pointer differs from source layout",
        )
    }

    pub(super) fn validate_compound(&self) -> Result<()> {
        let is_comet = self.model == ReferenceModel::CometUsdc;
        let native = self.model == ReferenceModel::CompoundV2Ceth2019;
        let expected_market = bytes(if is_comet {
            COMET
        } else if native {
            CETH
        } else {
            CUSDC
        });
        let basis = if is_comet { pb::BasisKind::SignedPrincipal } else { pb::BasisKind::Shares };
        require(
            self.epoch.chain_id == 1
                && self.epoch.market == expected_market
                && self.epoch.family
                    == if is_comet {
                        pb::ModelFamily::CompoundV3Comet
                    } else {
                        pb::ModelFamily::CompoundV2Ctoken
                    } as i32
                && self.epoch.model_id
                    == if is_comet {
                        "comet/base-supply-index"
                    } else {
                        "compound-v2/ctoken-2019/exchange-rate-stored"
                    }
                && pinned(&self.epoch.source_pin, if is_comet { COMET_SOURCE } else { CTOKEN_SOURCE })
                && self.epoch.basis_kind == basis as i32
                && self.epoch.basis_signed == is_comet
                && self.epoch.basis_bit_offset == 0
                && self.epoch.basis_bit_width == if is_comet { 104 } else { 256 }
                && self.epoch.basis_scale == if is_comet { INDEX } else { EXP }
                && self.epoch.balance_rounding == pb::Rounding::Floor as i32
                && self.epoch.balance_asset == if native { vec![] } else { bytes(USDC) }
                && self.epoch.balance_decimals == if native { 18 } else { 6 }
                && self.holder_binding.storage_contract == expected_market
                && self.holder_binding.mapping_slot == slot(if is_comet { 5 } else { 15 }),
            "unsupported Compound market, model, source, basis or asset units",
        )?;
        let mut shapes = Vec::new();
        let market = self.epoch.market.clone();
        let mut add = |field, contract: Vec<u8>, storage_slot, bit_offset, bit_width, key, constant| {
            shapes.push(InputBinding {
                field,
                key,
                storage_contract: contract,
                storage_slot,
                bit_offset,
                bit_width,
                observation: if constant {
                    pb::Observation::QualifiedConstant
                } else {
                    pb::Observation::ObservedWrite
                },
            });
        };
        use pb::StateField as F;
        if is_comet {
            require(
                self.dependencies.len() == 2 && self.epoch.implementation.len() == 20 && self.epoch.implementation_slot == bytes(EIP1967),
                "Comet implementation binding required",
            )?;
            let implementation = self.dependency(pb::DependencyRole::Implementation, 1, COMET_SOURCE)?;
            require(implementation.contract == self.epoch.implementation, "Comet implementation mismatch")?;
            self.pointer(implementation, &market, &bytes(EIP1967), &[])?;
            let underlying = self.dependency(pb::DependencyRole::Underlying, 1, COMET_SOURCE)?;
            require(
                underlying.contract == bytes(USDC)
                    && underlying.binding == pb::BindingKind::Declared as i32
                    && underlying.parent.is_empty()
                    && underlying.pointer_contract.is_empty()
                    && underlying.pointer_slot.is_empty()
                    && underlying.pointer_value.is_empty(),
                "Comet base asset declaration mismatch",
            )?;
            for (field, word, offset, width) in [
                (F::CometBaseSupplyIndex, 0, 0, 64),
                (F::CometBaseBorrowIndex, 0, 64, 64),
                (F::CometTotalSupplyBase, 1, 0, 104),
                (F::CometTotalBorrowBase, 1, 104, 104),
                (F::CometLastAccrualTime, 1, 208, 40),
                (F::CometPauseFlags, 1, 248, 8),
            ] {
                add(field, market.clone(), slot(word), offset, width, vec![], false);
            }
            for field in [
                F::CometSupplyKink,
                F::CometSupplyRateSlopeLow,
                F::CometSupplyRateSlopeHigh,
                F::CometSupplyRateBase,
                F::CometBorrowKink,
                F::CometBorrowRateSlopeLow,
                F::CometBorrowRateSlopeHigh,
                F::CometBorrowRateBase,
                F::CometBaseScale,
                F::CometBaseIndexScale,
                F::CometFactorScale,
            ] {
                add(field, implementation.contract.clone(), vec![], 0, 0, vec![], true);
            }
        } else {
            require(
                self.epoch.implementation.is_empty() && self.epoch.implementation_slot.is_empty() && self.dependencies.len() == if native { 1 } else { 3 },
                "only selected non-delegator cTokens are supported",
            )?;
            let irm = self.dependency(pb::DependencyRole::InterestRateModel, 1, if native { WHITE_SOURCE } else { JUMP_SOURCE })?;
            require(
                irm.contract
                    == bytes(if native {
                        "c64c4cba055efa614ce01f4bad8a9f519c4f8fab"
                    } else {
                        "d8ec56013ea119e7181d231e5048f90fbbe753c0"
                    }),
                "unqualified rate-model address",
            )?;
            self.pointer(irm, &market, &slot(7), &[])?;
            for (field, word) in [
                (F::CompoundV2InitialExchangeRateMantissa, 8),
                (F::CompoundV2ReserveFactorMantissa, 9),
                (F::CompoundV2AccrualBlockNumber, 10),
                (F::CompoundV2BorrowIndex, 11),
                (F::CompoundV2TotalBorrows, 12),
                (F::CompoundV2TotalReserves, 13),
                (F::CompoundV2TotalSupply, 14),
            ] {
                add(field, market.clone(), slot(word), 0, 256, vec![], false);
            }
            add(F::CompoundV2IrmBlocksPerYear, irm.contract.clone(), vec![], 0, 0, vec![], true);
            if native {
                for field in [F::CompoundV2IrmBaseRatePerYear, F::CompoundV2IrmMultiplierPerYear] {
                    add(field, irm.contract.clone(), vec![], 0, 0, vec![], true);
                }
                add(F::CompoundV2TotalCash, market.clone(), vec![], 0, 256, market.clone(), false);
            } else {
                for (field, word) in [
                    (F::CompoundV2IrmMultiplierPerBlock, 1),
                    (F::CompoundV2IrmBaseRatePerBlock, 2),
                    (F::CompoundV2IrmJumpMultiplierPerBlock, 3),
                    (F::CompoundV2IrmKink, 4),
                ] {
                    add(field, irm.contract.clone(), slot(word), 0, 256, vec![], false);
                }
                let underlying = self.dependency(pb::DependencyRole::Underlying, 1, USDC_SOURCE)?;
                require(underlying.contract == bytes(USDC), "unqualified cUSDC underlying")?;
                self.pointer(underlying, &market, &slot(18), &[])?;
                let implementation = self.dependency(pb::DependencyRole::Implementation, 2, USDC_SOURCE)?;
                self.pointer(implementation, &underlying.contract, &bytes(USDC_IMPL), &underlying.contract)?;
                add(
                    F::CompoundV2TotalCash,
                    underlying.contract.clone(),
                    holder_storage_slot(&market, &slot(9))?,
                    0,
                    255,
                    market.clone(),
                    false,
                );
            }
        }
        for (i, binding) in self.inputs.iter().enumerate() {
            require(
                !self.inputs[..i].iter().any(|other| other.field == binding.field) && shapes.contains(binding),
                "Compound input binding differs from source layout",
            )?;
        }
        Ok(())
    }

    pub(super) fn evaluate_compound(&self, ledger: &ProtocolLedger, holder: &[u8], metric: Metric, clock: pb::BlockClock) -> Result<Evaluation> {
        let is_comet = self.model == ReferenceModel::CometUsdc;
        let needs_holder = match (is_comet, metric) {
            (true, Metric::CometPrincipal | Metric::CometBalanceOf)
            | (false, Metric::HolderBasis | Metric::CompoundV2StoredUnderlying | Metric::CompoundV2ProjectedUnderlying) => true,
            (true, Metric::CometStoredSupplyIndex | Metric::CometProjectedSupplyIndex) | (false, Metric::CompoundV2ExchangeRateStored) => false,
            _ => return Err(Unknown::Invalid("metric does not belong to the qualified Compound model")),
        };
        let units = match metric {
            Metric::HolderBasis => MetricUnits::RawBasis(pb::BasisKind::Shares),
            Metric::CometPrincipal => MetricUnits::RawBasis(pb::BasisKind::SignedPrincipal),
            Metric::CompoundV2ExchangeRateStored => MetricUnits::ConversionRate { scale: EXP },
            Metric::CometStoredSupplyIndex | Metric::CometProjectedSupplyIndex => MetricUnits::Index { scale: INDEX },
            _ => MetricUnits::Asset {
                contract: self.epoch.balance_asset.clone(),
                decimals: self.epoch.balance_decimals,
            },
        };
        let mut result = Evaluation {
            metric,
            value: MetricValue::Unsigned(BigUint::zero()),
            units,
            clock: clock.clone(),
            model: self.epoch.clone(),
            qualification_evidence: self.evidence.clone(),
            runtime_qualification: self.runtime.clone().expect("validated"),
            holder: None,
            globals: vec![],
        };
        let mut principal = None;
        let mut shares = None;
        if needs_holder {
            let fact = ledger
                .holder(&self.epoch.market, holder)
                .map_err(|_| Unknown::MissingInput("initialized holder basis"))?;
            require(
                fact.row.basis_kind == self.epoch.basis_kind
                    && fact.row.signed == is_comet
                    && fact.row.bit_offset == 0
                    && fact.row.bit_width == self.epoch.basis_bit_width
                    && fact.row.storage_contract == self.epoch.market
                    && fact.row.storage_slot == holder_storage_slot(holder, &self.holder_binding.mapping_slot)?,
                "Compound holder provenance mismatch",
            )?;
            if is_comet {
                let value: BigInt = fact.row.value.parse().map_err(|_| Unknown::Invalid("signed principal required"))?;
                require(comet::fits_int104(&value), "principal exceeds int104")?;
                principal = Some(value);
            } else {
                let value = number(&fact.row.value)?;
                require(value.bits() <= 256, "shares exceed uint256")?;
                shares = Some(value);
            }
            result.holder = Some(fact.clone());
        }
        if metric == Metric::CometPrincipal {
            result.value = MetricValue::SignedPrincipal(principal.unwrap());
            return Ok(result);
        }
        if metric == Metric::HolderBasis {
            result.value = MetricValue::Unsigned(shares.unwrap());
            return Ok(result);
        }
        let mut read = |field, scale, observation| self.read(ledger, field, scale, observation, &mut result.globals);
        use pb::StateField as F;
        let stored = pb::Observation::ObservedWrite;
        let constant = pb::Observation::QualifiedConstant;
        let value = if is_comet {
            let supply = uint64(read(F::CometBaseSupplyIndex, INDEX, stored)?)?;
            if metric == Metric::CometStoredSupplyIndex {
                BigUint::from(supply)
            } else {
                require(clock.timestamp < (1 << 40), "Comet timestamp exceeds uint40")?;
                let rates = comet::RateModel {
                    supply_kink: uint64(read(F::CometSupplyKink, EXP, constant)?)?,
                    supply_slope_low: uint64(read(F::CometSupplyRateSlopeLow, EXP, constant)?)?,
                    supply_slope_high: uint64(read(F::CometSupplyRateSlopeHigh, EXP, constant)?)?,
                    supply_base: uint64(read(F::CometSupplyRateBase, EXP, constant)?)?,
                    borrow_kink: uint64(read(F::CometBorrowKink, EXP, constant)?)?,
                    borrow_slope_low: uint64(read(F::CometBorrowRateSlopeLow, EXP, constant)?)?,
                    borrow_slope_high: uint64(read(F::CometBorrowRateSlopeHigh, EXP, constant)?)?,
                    borrow_base: uint64(read(F::CometBorrowRateBase, EXP, constant)?)?,
                };
                require(
                    read(F::CometBaseScale, "1", constant)? == BigUint::from(1_000_000u64)
                        && read(F::CometBaseIndexScale, "1", constant)? == number(INDEX)?
                        && read(F::CometFactorScale, "1", constant)? == number(EXP)?,
                    "Comet scale constants differ from source",
                )?;
                let market = comet::Market {
                    base_supply_index: supply,
                    base_borrow_index: uint64(read(F::CometBaseBorrowIndex, INDEX, stored)?)?,
                    total_supply_base: read(F::CometTotalSupplyBase, "1", stored)?,
                    total_borrow_base: read(F::CometTotalBorrowBase, "1", stored)?,
                    last_accrual_time: uint64(read(F::CometLastAccrualTime, "1", stored)?)?,
                    rates,
                };
                if metric == Metric::CometProjectedSupplyIndex {
                    BigUint::from(market.accrued_indices(clock.timestamp)?.0)
                } else {
                    comet::balance_of(principal.as_ref().unwrap(), &market, clock.timestamp)?
                }
            }
        } else {
            // This bounded adapter requires a complete initialized market,
            // including the initial-rate branch. No absent word is fabricated.
            let market = v2::Market {
                total_cash: read(F::CompoundV2TotalCash, "1", stored)?,
                total_borrows: read(F::CompoundV2TotalBorrows, "1", stored)?,
                total_reserves: read(F::CompoundV2TotalReserves, "1", stored)?,
                total_supply: read(F::CompoundV2TotalSupply, "1", stored)?,
                borrow_index: read(F::CompoundV2BorrowIndex, EXP, stored)?,
                accrual_block_number: uint64(read(F::CompoundV2AccrualBlockNumber, "1", stored)?)?,
                reserve_factor_mantissa: read(F::CompoundV2ReserveFactorMantissa, EXP, stored)?,
                initial_exchange_rate_mantissa: read(F::CompoundV2InitialExchangeRateMantissa, EXP, stored)?,
            };
            if metric == Metric::CompoundV2ProjectedUnderlying {
                require(
                    read(F::CompoundV2IrmBlocksPerYear, "1", constant)? == BigUint::from(v2::BLOCKS_PER_YEAR),
                    "unexpected blocks-per-year constant",
                )?;
                let rates = if self.model == ReferenceModel::CompoundV2Ceth2019 {
                    v2::RateModel::WhitePaper2019(v2::WhitePaper2019 {
                        base_rate_per_year: read(F::CompoundV2IrmBaseRatePerYear, EXP, constant)?,
                        multiplier_per_year: read(F::CompoundV2IrmMultiplierPerYear, EXP, constant)?,
                    })
                } else {
                    v2::RateModel::Jump(v2::JumpRateModelV2 {
                        base_rate_per_block: read(F::CompoundV2IrmBaseRatePerBlock, EXP, stored)?,
                        multiplier_per_block: read(F::CompoundV2IrmMultiplierPerBlock, EXP, stored)?,
                        jump_multiplier_per_block: read(F::CompoundV2IrmJumpMultiplierPerBlock, EXP, stored)?,
                        kink: read(F::CompoundV2IrmKink, EXP, stored)?,
                    })
                };
                v2::underlying_balance_with(shares.as_ref().unwrap(), &market, &rates, v2::CTokenRevision::Legacy2019, clock.number)?
            } else {
                let rate = market.exchange_rate_stored()?;
                if metric == Metric::CompoundV2ExchangeRateStored {
                    rate
                } else {
                    let product = shares.as_ref().unwrap() * rate;
                    require(product.bits() <= 256, "stored underlying conversion multiplication overflow")?;
                    product / number(EXP)?
                }
            }
        };
        result.value = MetricValue::Unsigned(value);
        Ok(result)
    }
}
