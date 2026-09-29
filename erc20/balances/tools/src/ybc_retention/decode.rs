//! Branch-aware raw input planning. A missing word is never replaced by zero.
//! Only fields proven unreachable by a resolved early exit/skip are omitted.
use super::{binding, value, word, Address, At, Fact, Ledger, Slot};
use crate::ybc_rewards::{Hour, State};
use anyhow::Result;
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Metric<T> {
    Known(T),
    Unknown { missing: Vec<Slot> },
    ModelRefusal { reason: String },
    ScopeRefusal { reason: String },
    Suspended { epoch: u32, reason: String },
}
impl<T> Metric<T> {
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Unknown { .. })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reward {
    pub pending_reward: String,
    pub stopping_hour: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evaluation {
    pub holder: Address,
    pub at: At,
    pub binding_sha256: String,
    pub input_sha256: String,
    pub raw_basis: Metric<String>,
    pub pending: Metric<Reward>,
    pub observable: Metric<String>,
    /// Exact accessed raw facts, with their independent origins. Whole-state digest
    /// additionally binds the finite registry and all retained but unused facts.
    pub used: Vec<Fact>,
}
pub fn map(key: U256, root: U256) -> [u8; 32] {
    let mut pre = [0; 64];
    key.to_big_endian(&mut pre[..32]);
    root.to_big_endian(&mut pre[32..]);
    erc20_balances::hash(&pre)
}
pub fn holder_key(holder: Address, root: u64, offset: u64) -> Slot {
    let base = U256::from_big_endian(&map(U256::from_big_endian(&holder), root.into()));
    Slot {
        contract: binding::token(),
        key: word(base.overflowing_add(offset.into()).0),
    }
}
pub fn scalar(contract: Address, key: u64) -> Slot {
    Slot { contract, key: word(key) }
}
pub fn hour_key(holder: Address, hour: U256, root: u64) -> Slot {
    let root = if root == 26 {
        U256::from_big_endian(&map(U256::from_big_endian(&holder), 26.into()))
    } else {
        root.into()
    };
    Slot {
        contract: binding::token(),
        key: map(hour, root),
    }
}
type Failure = Metric<Reward>;
struct Reader<'a> {
    ledger: &'a Ledger,
    used: BTreeMap<Slot, Fact>,
}
impl Reader<'_> {
    fn get(&mut self, slot: Slot) -> std::result::Result<U256, Failure> {
        let Some(f) = self.ledger.fact(&slot) else {
            return Err(Metric::Unknown { missing: vec![slot] });
        };
        self.used.insert(slot, f.clone());
        Ok(value(&f.word))
    }
    fn model<T>(&self, r: anyhow::Result<T>) -> std::result::Result<T, Failure> {
        r.map_err(|e| Metric::ModelRefusal { reason: e.to_string() })
    }
    fn reward(&mut self, h: Address) -> std::result::Result<Reward, Failure> {
        let pointer = self.get(binding::pointer())?;
        if pointer != value(&binding::helper_word()) {
            return Err(Metric::ScopeRefusal {
                reason: "helper pointer outside reviewed identity".into(),
            });
        }
        let rate = self.get(holder_key(h, 22, 4))?;
        let zero = || Reward {
            pending_reward: "0".into(),
            stopping_hour: "0".into(),
        };
        if rate < 1_000_000_000u64.into() {
            return Ok(zero());
        }
        let launch = self.get(scalar(binding::token(), 31))?;
        let now = U256::from(self.ledger.at().timestamp);
        let current = now.checked_sub(launch).ok_or_else(|| Metric::ModelRefusal {
            reason: "uint256 subtraction underflow".into(),
        })? / 3600;
        if current.is_zero() {
            return Ok(zero());
        }
        let last = self.get(holder_key(h, 23, 0))?;
        if last >= current {
            return Ok(zero());
        }
        let stop = current.min(last.checked_add(240.into()).ok_or_else(|| Metric::ModelRefusal {
            reason: "uint256 addition overflow".into(),
        })?);
        // This subtraction is <=240 in U256 before any host conversion. Neither a
        // huge stored cursor nor a huge gap can allocate an unbounded scan.
        let count = (stop - last).as_usize();
        let initial_total = self.get(hour_key(h, last, 28))?;
        let initial_user = self.get(hour_key(h, last, 26))?;
        // The recorded helper calls pool balanceOf before the first hour. Its own
        // active reward path is outside the captured terminal-pool model.
        let pool_rate = self.get(holder_key(binding::pool(), 22, 4))?;
        if pool_rate >= 1_000_000_000u64.into() {
            return Err(Metric::ScopeRefusal {
                reason: "pool reward recursion outside reviewed model".into(),
            });
        }
        let pool_balance = self.get(holder_key(binding::pool(), 0, 0))?;
        let mut hours = Vec::with_capacity(count);
        let mut carried_total = initial_total;
        let mut needs_burn = false;
        for i in 0..count {
            let hour = last + U256::from(i);
            let no_burn = !(self.get(hour_key(h, hour, 49))? & U256::from(255)).is_zero();
            if no_burn {
                // Pure model checks first-hour identity before its skip. These are the
                // already observed initial values, not inferred storage for the skipped row.
                hours.push(Hour {
                    no_burn: true,
                    total_rate: if i == 0 { initial_total } else { U256::zero() },
                    user_rate: if i == 0 { initial_user } else { U256::zero() },
                    reward: U256::zero(),
                });
            } else {
                let total = self.get(hour_key(h, hour, 28))?;
                if !total.is_zero() {
                    carried_total = total;
                }
                let reward = self.get(hour_key(h, hour, 29))?;
                let user = self.get(hour_key(h, hour, 26))?;
                needs_burn |= reward.is_zero() && !carried_total.is_zero() && !pool_balance.is_zero();
                hours.push(Hour {
                    no_burn: false,
                    total_rate: total,
                    user_rate: user,
                    reward,
                });
            }
        }
        // No preview can use this value when all recorded rewards are nonzero or
        // pool balance is zero; zero here is an unreachable local argument, not fact.
        let burn = if needs_burn { self.get(scalar(binding::token(), 36))? } else { U256::zero() };
        let reserves = self.get(scalar(binding::pool(), 8))?;
        let token1 = self.get(scalar(binding::pool(), 7))?;
        let wbnb = U256::from_big_endian(&super::address(binding::WBNB).unwrap());
        if token1 != wbnb && token1 != U256::from_big_endian(&binding::token()) {
            return Err(Metric::ScopeRefusal {
                reason: "pool orientation outside reviewed pair".into(),
            });
        }
        let mask = (U256::one() << 112) - 1;
        let (r0, r1) = (reserves & mask, (reserves >> 112) & mask);
        let state = State {
            raw: U256::zero(),
            static_rate: rate,
            wbnb_value: self.get(holder_key(h, 22, 3))?,
            lp_reward: self.get(holder_key(h, 22, 5))?,
            now,
            launch_time: launch,
            last_cycle: last,
            initial_total_rate: initial_total,
            initial_user_rate: initial_user,
            pool_balance,
            burn_per_day: burn,
            token_reserve: if token1 == wbnb { r0 } else { r1 },
            quote_reserve: if token1 == wbnb { r1 } else { r0 },
            hours,
        };
        let (amount, stop) = self.model(state.reward())?;
        Ok(Reward {
            pending_reward: amount.to_string(),
            stopping_hour: stop.to_string(),
        })
    }
}
fn propagate<T>(m: &Metric<Reward>) -> Metric<T> {
    match m {
        Metric::Unknown { missing } => Metric::Unknown { missing: missing.clone() },
        Metric::ModelRefusal { reason } => Metric::ModelRefusal { reason: reason.clone() },
        Metric::ScopeRefusal { reason } => Metric::ScopeRefusal { reason: reason.clone() },
        Metric::Suspended { epoch, reason } => Metric::Suspended {
            epoch: *epoch,
            reason: reason.clone(),
        },
        Metric::Known(_) => unreachable!(),
    }
}
pub fn evaluate(l: &Ledger, h: Address, input_sha256: String) -> Result<Evaluation> {
    let mut r = Reader {
        ledger: l,
        used: BTreeMap::new(),
    };
    let raw = r.get(holder_key(h, 0, 0));
    let raw_basis = match &raw {
        Ok(v) => Metric::Known(v.to_string()),
        Err(m) => propagate(m),
    };
    let pending = if let Some(reason) = l.suspension() {
        Metric::Suspended {
            epoch: l.binding().epoch,
            reason: reason.into(),
        }
    } else {
        match r.reward(h) {
            Ok(v) => Metric::Known(v),
            Err(m) => m,
        }
    };
    let observable = match (&pending, &raw) {
        (Metric::Known(p), Ok(raw)) => match raw.checked_add(U256::from_dec_str(&p.pending_reward)?) {
            Some(v) => Metric::Known(v.to_string()),
            None => Metric::ModelRefusal {
                reason: "uint256 addition overflow".into(),
            },
        },
        (Metric::Known(_), Err(m)) => propagate(m),
        (m, _) => propagate(m),
    };
    Ok(Evaluation {
        holder: h,
        at: l.at().clone(),
        binding_sha256: crate::calculated_retention::binding::sha(&serde_json::to_vec(l.binding())?),
        input_sha256,
        raw_basis,
        pending,
        observable,
        used: r.used.into_values().collect(),
    })
}
