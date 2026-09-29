//! Closed finite input planner for the unchanged OG arithmetic. Raw facts and
//! computed units stay separate. Missing history is never filled with zero.
use super::{binding, value, word, Address, At, Fact, Ledger, Slot};
use crate::og_model::{fixture, Period, State};
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
    pub amount: String,
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
    pub hourly: Metric<Reward>,
    pub daily: Metric<String>,
    pub observable: Metric<String>,
    pub used: Vec<Fact>,
}
pub fn holder_key(h: Address, root: u64, offset: u64) -> Slot {
    Slot {
        contract: binding::token(),
        key: word(fixture::key(&[U256::from_big_endian(&h)], root).overflowing_add(offset.into()).0),
    }
}
pub fn scalar(contract: Address, key: u64) -> Slot {
    Slot { contract, key: word(key) }
}
pub fn period_key(h: Address, p: U256, root: u64) -> Slot {
    let args = if matches!(root, 12 | 13) {
        vec![U256::from_big_endian(&h), p]
    } else {
        vec![p]
    };
    Slot {
        contract: binding::token(),
        key: word(fixture::key(&args, root)),
    }
}
type Failure = Metric<Reward>;
type PoolInputs = (U256, U256, Option<[U256; 4]>, U256);
fn model<T>(r: anyhow::Result<T>) -> std::result::Result<T, Failure> {
    r.map_err(|e| Metric::ModelRefusal { reason: e.to_string() })
}
fn failure(reason: &str) -> Failure {
    Metric::ModelRefusal { reason: reason.into() }
}
fn propagate<T>(m: &Failure) -> Metric<T> {
    match m {
        Metric::Known(_) => unreachable!(),
        Metric::Unknown { missing } => Metric::Unknown { missing: missing.clone() },
        Metric::ModelRefusal { reason } => Metric::ModelRefusal { reason: reason.clone() },
        Metric::ScopeRefusal { reason } => Metric::ScopeRefusal { reason: reason.clone() },
        Metric::Suspended { epoch, reason } => Metric::Suspended {
            epoch: *epoch,
            reason: reason.clone(),
        },
    }
}
struct Reader<'a> {
    l: &'a Ledger,
    used: BTreeMap<Slot, Fact>,
}
impl Reader<'_> {
    fn get(&mut self, s: Slot) -> std::result::Result<U256, Failure> {
        let f = self.l.fact(&s).ok_or_else(|| Metric::Unknown { missing: vec![s.clone()] })?;
        self.used.insert(s, f.clone());
        Ok(value(&f.word))
    }
    fn pointers(&mut self) -> std::result::Result<(), Failure> {
        for (s, a) in binding::pointers() {
            let w = word(self.get(s)?);
            if binding::low_address(&w) != a {
                return Err(Metric::ScopeRefusal {
                    reason: "unreviewed stored dependency address".into(),
                });
            }
        }
        Ok(())
    }
    fn user(&mut self, h: Address) -> std::result::Result<[U256; 13], Failure> {
        let p = self.get(holder_key(h, 9, 0))?;
        let mut u = [U256::zero(); 13];
        u[0] = p & ((U256::one() << 160) - 1);
        u[1] = (p >> 160) & U256::from(255);
        for (i, x) in u.iter_mut().enumerate().skip(2) {
            *x = self.get(holder_key(h, 9, (i - 1) as u64))?;
        }
        Ok(u)
    }
    fn elapsed(&mut self) -> std::result::Result<(U256, U256), Failure> {
        let epoch = self.get(scalar(binding::token(), 29))?;
        let elapsed = U256::from(self.l.at().timestamp)
            .checked_sub(epoch)
            .ok_or_else(|| failure("uint256 subtraction underflow"))?;
        Ok((epoch, elapsed))
    }
    fn pool(&mut self) -> std::result::Result<PoolInputs, Failure> {
        let u = self.user(binding::pool_a())?;
        let raw = self.get(holder_key(binding::pool_a(), 0, 0))?;
        if u[5].is_zero() {
            return Ok((u[5], u[6], None, raw));
        }
        let (_, elapsed) = self.elapsed()?;
        let cursors = [
            self.get(holder_key(binding::pool_a(), 11, 0))?,
            self.get(holder_key(binding::pool_a(), 11, 1))?,
            self.get(holder_key(binding::pool_a(), 11, 2))?,
            self.get(holder_key(binding::pool_a(), 11, 3))?,
        ];
        let hour = elapsed / 3600;
        let day = elapsed / 86400;
        if (!hour.is_zero() && cursors[0] < hour) || (!u[6].is_zero() && !day.is_zero() && cursors[1] < day) {
            return Err(Metric::ScopeRefusal {
                reason: "unmodeled recursive pool reward; no recursion or gas model".into(),
            });
        }
        Ok((u[5], u[6], Some(cursors), raw))
    }
    fn common(&mut self, h: Address, user: [U256; 13], epoch: U256) -> std::result::Result<State, Failure> {
        let (pool_hour_rate, pool_day_rate, pool_cursors, pool_balance) = self.pool()?;
        let a = self.get(scalar(binding::pool_a(), 8))?;
        let b = self.get(scalar(binding::pool_b(), 8))?;
        let mask = (U256::one() << 112) - 1;
        // Zero placeholders are only local arguments unreachable by the selected
        // public metric. They are never inserted as facts or returned as raw state.
        Ok(State {
            raw: U256::zero(),
            user,
            last_hour: U256::zero(),
            last_day: U256::zero(),
            now: U256::from(self.l.at().timestamp),
            epoch,
            amount40: self.get(holder_key(h, 40, 0))?,
            last39: self.get(holder_key(h, 39, 0))?,
            claimed10: self.get(holder_key(h, 10, 0))?,
            reserve0_a: a & mask,
            reserve1_a: (a >> 112) & mask,
            reserve0_b: b & mask,
            reserve1_b: (b >> 112) & mask,
            pool_hour_rate,
            pool_day_rate,
            pool_cursors,
            pool_balance,
            hours: vec![],
            days: vec![],
        })
    }
    fn periods(&mut self, h: Address, start: U256, count: usize, daily: bool) -> std::result::Result<Vec<Period>, Failure> {
        let (t, u, r) = if daily { (25, 13, 27) } else { (23, 12, 24) };
        (0..count)
            .map(|i| {
                let index = start.checked_add(U256::from(i)).ok_or_else(|| failure("uint256 addition overflow"))?;
                Ok(Period {
                    index,
                    total: self.get(period_key(h, index, t))?,
                    user: self.get(period_key(h, index, u))?,
                    reward: self.get(period_key(h, index, r))?,
                })
            })
            .collect()
    }
    fn hourly(&mut self, h: Address) -> std::result::Result<Reward, Failure> {
        self.pointers()?;
        let user = self.user(h)?;
        let zero = || Reward {
            amount: "0".into(),
            stopping_hour: "0".into(),
        };
        if user[5].is_zero() {
            return Ok(zero());
        }
        let (epoch, elapsed) = self.elapsed()?;
        let current = elapsed / 3600;
        if current.is_zero() {
            return Ok(zero());
        }
        let last = self.get(holder_key(h, 11, 0))?;
        if last >= current {
            return Ok(zero());
        }
        let stop = current.min(last.checked_add(168.into()).ok_or_else(|| failure("uint256 addition overflow"))?);
        let count = (stop - last).as_usize();
        let mut state = self.common(h, user, epoch)?;
        state.last_hour = last;
        state.hours = self.periods(h, last, count, false)?;
        let (amount, stopping_hour) = model(state.hourly())?;
        Ok(Reward {
            amount: amount.to_string(),
            stopping_hour: stopping_hour.to_string(),
        })
    }
    fn daily(&mut self, h: Address) -> std::result::Result<U256, Failure> {
        self.pointers()?;
        let user = self.user(h)?;
        if user[5].is_zero() || user[6].is_zero() {
            return Ok(U256::zero());
        }
        let (epoch, elapsed) = self.elapsed()?;
        let current = elapsed / 86400;
        if current.is_zero() {
            return Ok(U256::zero());
        }
        let last = self.get(holder_key(h, 11, 1))?;
        if last >= current {
            return Ok(U256::zero());
        }
        let count = current - last;
        if count > 1000.into() {
            return Err(Metric::ScopeRefusal {
                reason: "conservative daily planning exceeds1000 periods".into(),
            });
        }
        let mut state = self.common(h, user, epoch)?;
        state.last_day = last;
        // Conservatively require the full bounded horizon, even when the unchanged
        // model later stops after three positive periods. No truncated partial sum.
        state.days = self.periods(h, last, count.as_usize(), true)?;
        model(state.daily())
    }
}
pub fn pool_terminal(l: &Ledger) -> Metric<String> {
    if let Some(reason) = l.suspension() {
        return Metric::Suspended {
            epoch: l.binding().epoch,
            reason: reason.into(),
        };
    }
    let mut r = Reader { l, used: BTreeMap::new() };
    match r.pointers().and_then(|_| r.pool()) {
        Ok((_, _, _, v)) => Metric::Known(v.to_string()),
        Err(e) => propagate(&e),
    }
}
pub fn evaluate(l: &Ledger, h: Address, input_sha256: String) -> Result<Evaluation> {
    let mut r = Reader { l, used: BTreeMap::new() };
    let raw = r.get(holder_key(h, 0, 0));
    let raw_basis = match &raw {
        Ok(v) => Metric::Known(v.to_string()),
        Err(e) => propagate(e),
    };
    let suspended = || Metric::Suspended {
        epoch: l.binding().epoch,
        reason: l.suspension().unwrap().into(),
    };
    let hour = if l.suspension().is_some() { Err(suspended()) } else { r.hourly(h) };
    // Separate helper metrics are independent requests. The observable branch
    // below still checks raw+hourly before it observes the daily result.
    let day = if l.suspension().is_some() { Err(suspended()) } else { r.daily(h) };
    let hourly = match &hour {
        Ok(v) => Metric::Known(v.clone()),
        Err(e) => e.clone(),
    };
    let daily = match &day {
        Ok(v) => Metric::Known(v.to_string()),
        Err(e) => propagate(e),
    };
    let observable = match (&hour, &raw) {
        (Err(e), _) | (_, Err(e)) => propagate(e),
        (Ok(h), Ok(raw)) => match raw.checked_add(U256::from_dec_str(&h.amount)?) {
            None => propagate(&failure("uint256 addition overflow")),
            Some(subtotal) => match &day {
                Err(e) => propagate(e),
                Ok(day) => match subtotal.checked_add(*day) {
                    Some(v) => Metric::Known(v.to_string()),
                    None => propagate(&failure("uint256 addition overflow")),
                },
            },
        },
    };
    Ok(Evaluation {
        holder: h,
        at: l.at().clone(),
        binding_sha256: crate::calculated_retention::binding::sha(&serde_json::to_vec(l.binding())?),
        input_sha256,
        raw_basis,
        hourly,
        daily,
        observable,
        used: r.used.into_values().collect(),
    })
}
