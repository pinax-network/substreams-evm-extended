//! Independent raw-word expectations; reachable operations kept separate from storage perturbations.
use super::{vm, Target};
use crate::ptoken_proof::cases::{call, execution_json, get, mapping, role, state_json, w};
use anyhow::{ensure, Context, Result};
use primitive_types::U256;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use vm::{Execution, Exit, State};
pub fn balance(who: U256) -> U256 {
    mapping(who, 0.into())
}
pub fn allowance(a: U256, b: U256) -> U256 {
    mapping(b, mapping(a, 1.into()))
}
pub fn head(t: Target, r: U256) -> U256 {
    mapping(r, t.role_root())
}
pub fn member(t: Target, r: U256, m: U256) -> U256 {
    mapping(m, head(t, r) + U256::from(u8::from(t == Target::Dsg)))
}
pub fn full_role() -> U256 {
    U256::max_value() - 1
}
pub fn holders(t: Target) -> [U256; 4] {
    [
        U256::from_str_radix(
            if t == Target::Apd {
                "bc1b47c8905bdc908a051449d62f086f300460ff"
            } else {
                "fb26cf258ad722b5e4d1b806fd07bba043d0e437"
            },
            16,
        )
        .unwrap(),
        44.into(),
        t.account(),
        0.into(),
    ]
}
fn normalized(s: &State) -> State {
    s.iter().filter(|(_, v)| !v.is_zero()).map(|(k, v)| (*k, *v)).collect()
}
fn set_role(s: &mut State, t: Target, r: U256, members: &[U256]) {
    if t == Target::Dsg {
        s.insert(head(t, r), members.len().into());
    }
    for (i, m) in members.iter().enumerate() {
        s.insert(member(t, r, *m), if t == Target::Apd { 1.into() } else { (i + 1).into() });
        if t == Target::Dsg {
            s.insert(vm::hash(&vm::word(head(t, r))) + U256::from(i), *m);
        }
    }
}
pub fn base(t: Target) -> State {
    let mut s = State::new();
    set_role(&mut s, t, 0.into(), &[1.into()]);
    set_role(&mut s, t, full_role(), &[44.into()]);
    for (i, h) in holders(t).iter().enumerate() {
        s.insert(balance(*h), U256::from(123 + i));
    }
    s.insert(allowance(44.into(), 55.into()), 17.into());
    s.insert(mapping(44.into(), t.nonce_root()), 19.into());
    for slot in if t == Target::Apd { vec![2, 7, 10, 11] } else { vec![2, 6, 9, 11, 12] } {
        s.insert(slot.into(), U256::from(slot + 21));
    }
    s.insert(U256::max_value(), 79.into());
    s
}
fn error(s: &str) -> Vec<u8> {
    let mut v = call("Error(string)", &[32.into(), s.len().into()]);
    v.extend(s.as_bytes());
    v.resize(4 + (v.len() - 4).div_ceil(32) * 32, 0);
    v
}
#[derive(Clone)]
pub struct Measured {
    pub target: Target,
    pub name: String,
    pub before: State,
    pub execution: Execution,
}
pub struct Proof<'a> {
    pub target: Target,
    pub runtime: &'a [u8],
    pub records: Vec<Value>,
    pub operations: Vec<Measured>,
    pub raw_projections: Vec<Value>,
}
impl<'a> Proof<'a> {
    pub fn new(target: Target, runtime: &'a [u8]) -> Self {
        Self {
            target,
            runtime,
            records: vec![],
            operations: vec![],
            raw_projections: vec![],
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn run(
        &mut self,
        name: &str,
        scope: &str,
        sig: &str,
        data: &[u8],
        caller: U256,
        s: &State,
        save: &mut impl FnMut(&Value) -> Result<()>,
    ) -> Result<Execution> {
        let e = vm::execute(self.runtime, data, caller, self.target.account(), s);
        let v = json!({"name":name,"target":self.target.label(),"scope":scope,"signature":sig,"code_kind":"deployedBytecode","code_sha256":super::sha(self.runtime),"caller":w(caller),"address":w(self.target.account()),"calldata":hex::encode(data),"prestate":state_json(s),"execution":execution_json(&e)});
        save(&v)?;
        self.records.push(v);
        ensure!(
            !matches!(e.exit, Exit::Invalid | Exit::HarnessFailure(_)),
            "{name}: unexpected exit {:?}",
            e.exit
        );
        validate_witnesses(s, &e)?;
        Ok(e)
    }
    #[allow(clippy::too_many_arguments)]
    fn getter(
        &mut self,
        name: &str,
        sig: &str,
        args: &[U256],
        caller: U256,
        s: &State,
        value: U256,
        reads: &[U256],
        save: &mut impl FnMut(&Value) -> Result<()>,
    ) -> Result<Execution> {
        let e = self.run(name, "synthetic raw-storage getter control", sig, &call(sig, args), caller, s, save)?;
        ensure!(
            e.exit == Exit::Return(vm::word(value).to_vec()),
            "{name}: exact getter return {:?}, expected {}",
            e.exit,
            value
        );
        ensure!(
            e.writes.is_empty() && e.logs.is_empty() && normalized(&e.committed) == normalized(s),
            "getter is read-only"
        );
        ensure!(
            e.reads.iter().map(|r| r.key).collect::<BTreeSet<_>>() == reads.iter().copied().collect(),
            "{name}: exact storage read cells"
        );
        Ok(e)
    }
    fn balances(&mut self, name: &str, s: &State, save: &mut impl FnMut(&Value) -> Result<()>) -> Result<()> {
        for caller in [1.into(), 77.into()] {
            for who in holders(self.target) {
                self.getter(name, "balanceOf(address)", &[who], caller, s, get(s, balance(who)), &[balance(who)], save)?;
            }
        }
        Ok(())
    }
    pub fn getters(&mut self, save: &mut impl FnMut(&Value) -> Result<()>) -> Result<()> {
        let t = self.target;
        for who in holders(t) {
            for val in [U256::zero(), 1.into(), 123.into(), U256::max_value()] {
                let mut s = base(t);
                s.insert(balance(who), val);
                for caller in [1.into(), 77.into()] {
                    self.getter("balance word domain", "balanceOf(address)", &[who], caller, &s, val, &[balance(who)], save)?;
                }
            }
        }
        let mut groups: Vec<(&str, Vec<U256>)> = vec![
            ("allowance", vec![allowance(44.into(), 55.into())]),
            ("nonce", vec![mapping(44.into(), t.nonce_root())]),
        ];
        for (n, slot) in if t == Target::Apd {
            vec![("supply", 2), ("pair", 7), ("buy", 10), ("sell", 11)]
        } else {
            vec![("supply", 2), ("burnt", 6), ("pair", 9), ("sell", 11), ("buy", 12)]
        } {
            groups.push((n, vec![slot.into()]));
        }
        for (name, keys) in &groups {
            for value in [U256::zero(), 1.into(), U256::max_value()] {
                let mut s = base(t);
                let value = if *name == "pair" { value & ((U256::one() << 160) - 1) } else { value };
                for k in keys {
                    s.insert(*k, value);
                }
                self.metadata(&format!("{name} perturbation"), &s, save)?;
                self.balances(&format!("balance independent of {name}"), &s, save)?;
                self.raw_projection(name, &base(t), &s, keys, save)?;
            }
        }
        for members in [vec![], vec![U256::zero()], vec![U256::zero(), 44.into(), 55.into()]] {
            let mut s = base(t);
            s.remove(&member(t, full_role(), 44.into()));
            if t == Target::Dsg {
                s.remove(&vm::hash(&vm::word(head(t, full_role()))));
            }
            set_role(&mut s, t, full_role(), &members);
            self.metadata("canonical role perturbation", &s, save)?;
            self.balances("balance independent of roles", &s, save)?;
        }
        let mut s = base(t);
        let all_keys = groups.iter().flat_map(|(_, keys)| keys.iter().copied()).collect::<Vec<_>>();
        for (name, keys) in groups {
            for k in keys {
                s.insert(k, if name == "pair" { 55.into() } else { U256::max_value() });
            }
        }
        set_role(&mut s, t, full_role(), &[0.into(), 44.into(), 55.into()]);
        self.metadata("combined metadata", &s, save)?;
        self.balances("balance independent of combined metadata", &s, save)?;
        self.raw_projection("combined non-role metadata", &base(t), &s, &all_keys, save)?;
        Ok(())
    }
    fn raw_projection(&mut self, name: &str, before: &State, after: &State, keys: &[U256], save: &mut impl FnMut(&Value) -> Result<()>) -> Result<()> {
        let who = holders(self.target)[0];
        let mut state = before.clone();
        for key in keys {
            state.insert(*key, get(after, *key));
        }
        state.insert(balance(who), 987.into());
        let getter = self.getter(
            "mixed raw metadata balance getter",
            "balanceOf(address)",
            &[who],
            77.into(),
            &state,
            987.into(),
            &[balance(who)],
            save,
        )?;
        self.raw_projections
            .push(super::projector::verify_raw_metadata(self.target, name, before, after, keys, &getter)?);
        Ok(())
    }
    fn metadata(&mut self, name: &str, s: &State, save: &mut impl FnMut(&Value) -> Result<()>) -> Result<()> {
        let t = self.target;
        for caller in [1.into(), 77.into()] {
            for (sig, args, key) in [
                ("allowance(address,address)", vec![44.into(), 55.into()], allowance(44.into(), 55.into())),
                ("nonces(address)", vec![44.into()], mapping(44.into(), t.nonce_root())),
            ] {
                self.getter(name, sig, &args, caller, s, get(s, key), &[key], save)?;
            }
            for (sig, slot) in if t == Target::Apd {
                vec![("totalSupply()", 2), ("mainPair()", 7), ("swapBuyTaxRatio()", 10), ("swapSellTaxRatio()", 11)]
            } else {
                vec![
                    ("totalSupply()", 2),
                    ("totalBurnt()", 6),
                    ("mainPair()", 9),
                    ("sellFeeRatio()", 11),
                    ("buyFeeRatio()", 12),
                ]
            } {
                self.getter(name, sig, &[], caller, s, get(s, slot.into()), &[slot.into()], save)?;
            }
            for who in [0.into(), 44.into(), 55.into()] {
                let key = member(t, full_role(), who);
                self.getter(
                    name,
                    "hasRole(bytes32,address)",
                    &[full_role(), who],
                    caller,
                    s,
                    u8::from(!get(s, key).is_zero()).into(),
                    &[key],
                    save,
                )?;
            }
            if t == Target::Dsg {
                let h = head(t, full_role());
                let len = get(s, h);
                self.getter(name, "getRoleMemberCount(bytes32)", &[full_role()], caller, s, len, &[h], save)?;
                for i in 0..len.as_usize() {
                    let key = vm::hash(&vm::word(h)) + U256::from(i);
                    self.getter(
                        name,
                        "getRoleMember(bytes32,uint256)",
                        &[full_role(), i.into()],
                        caller,
                        s,
                        get(s, key),
                        &[h, key],
                        save,
                    )?;
                }
            }
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn operation(
        &mut self,
        name: &str,
        sig: &str,
        args: &[U256],
        caller: U256,
        s: &State,
        exit: Exit,
        writes: &[(U256, U256)],
        logs: &[(Vec<U256>, Vec<u8>)],
        save: &mut impl FnMut(&Value) -> Result<()>,
    ) -> Result<Execution> {
        let e = self.run(
            name,
            "synthetic local source operation; caller and authorization state supplied",
            sig,
            &call(sig, args),
            caller,
            s,
            save,
        )?;
        check_operation(s, &e, &exit, writes, logs)?;
        self.operations.push(Measured {
            target: self.target,
            name: name.into(),
            before: s.clone(),
            execution: e.clone(),
        });
        self.balances(&format!("balance after {name}"), &e.committed, save)?;
        Ok(e)
    }
    pub fn apd_operations(&mut self, save: &mut impl FnMut(&Value) -> Result<()>) -> Result<()> {
        ensure!(self.target == Target::Apd, "APD only");
        let t = self.target;
        for r in [role("TEST_ROLE"), full_role()] {
            for who in [U256::zero(), 44.into()] {
                for (grant, present) in [(true, false), (true, true), (false, true), (false, false)] {
                    let mut s = base(t);
                    s.insert(member(t, r, who), u8::from(present).into());
                    let sig = if grant { "grantRole(bytes32,address)" } else { "revokeRole(bytes32,address)" };
                    let change = grant != present;
                    let writes = if change { vec![(member(t, r, who), u8::from(grant).into())] } else { vec![] };
                    let logs = if change {
                        vec![(
                            vec![
                                role(if grant {
                                    "RoleGranted(bytes32,address,address)"
                                } else {
                                    "RoleRevoked(bytes32,address,address)"
                                }),
                                r,
                                who,
                                1.into(),
                            ],
                            vec![],
                        )]
                    } else {
                        vec![]
                    };
                    let e = self.operation("APD role edge", sig, &[r, who], 1.into(), &s, Exit::Return(vec![]), &writes, &logs, save)?;
                    self.getter(
                        "role postcondition",
                        "hasRole(bytes32,address)",
                        &[r, who],
                        77.into(),
                        &e.committed,
                        u8::from(grant).into(),
                        &[member(t, r, who)],
                        save,
                    )?;
                    self.operation(
                        "APD unauthorized role",
                        sig,
                        &[r, who],
                        77.into(),
                        &s,
                        Exit::Revert(call("AccessControlUnauthorizedAccount(address,bytes32)", &[77.into(), 0.into()])),
                        &[],
                        &[],
                        save,
                    )?;
                }
                let mut s = base(t);
                s.insert(member(t, r, who), 1.into());
                let e = self.operation(
                    "APD renounce",
                    "renounceRole(bytes32,address)",
                    &[r, who],
                    who,
                    &s,
                    Exit::Return(vec![]),
                    &[(member(t, r, who), 0.into())],
                    &[(vec![role("RoleRevoked(bytes32,address,address)"), r, who, who], vec![])],
                    save,
                )?;
                self.getter(
                    "renounce postcondition",
                    "hasRole(bytes32,address)",
                    &[r, who],
                    77.into(),
                    &e.committed,
                    0.into(),
                    &[member(t, r, who)],
                    save,
                )?;
                self.operation(
                    "APD wrong confirmation",
                    "renounceRole(bytes32,address)",
                    &[r, who],
                    77.into(),
                    &s,
                    Exit::Revert(call("AccessControlBadConfirmation()", &[])),
                    &[],
                    &[],
                    save,
                )?;
            }
        }
        for amount in [0.into(), 123.into(), U256::max_value()] {
            let s = base(t);
            let e = self.operation(
                "APD approve",
                "approve(address,uint256)",
                &[55.into(), amount],
                44.into(),
                &s,
                Exit::Return(vm::word(1.into()).to_vec()),
                &[(allowance(44.into(), 55.into()), amount)],
                &[(vec![role("Approval(address,address,uint256)"), 44.into(), 55.into()], vm::word(amount).to_vec())],
                save,
            )?;
            self.getter(
                "allowance postcondition",
                "allowance(address,address)",
                &[44.into(), 55.into()],
                77.into(),
                &e.committed,
                amount,
                &[allowance(44.into(), 55.into())],
                save,
            )?;
        }
        for (caller, spender, msg) in [
            (U256::zero(), 55.into(), "ERC20: approve from the zero address"),
            (44.into(), 0.into(), "ERC20: approve to the zero address"),
        ] {
            self.operation(
                "APD zero approval",
                "approve(address,uint256)",
                &[spender, 7.into()],
                caller,
                &base(t),
                Exit::Revert(error(msg)),
                &[],
                &[],
                save,
            )?;
        }
        for (buy, sell, ok) in [(0, 0, true), (10000, 9999, true), (10001, 0, false), (0, 10000, false)] {
            let s = base(t);
            let writes = if ok {
                vec![(10.into(), buy.into()), (11.into(), sell.into())]
            } else {
                vec![]
            };
            let logs = if ok {
                vec![(
                    vec![role("FeeRatioChanged(uint256,uint256)")],
                    [vm::word(buy.into()), vm::word(sell.into())].concat(),
                )]
            } else {
                vec![]
            };
            let e = self.operation(
                "APD setter boundary",
                "setSwapTaxRatio(uint256,uint256)",
                &[buy.into(), sell.into()],
                1.into(),
                &s,
                if ok { Exit::Return(vec![]) } else { Exit::Revert(error("tax too high!")) },
                &writes,
                &logs,
                save,
            )?;
            for (sig, slot, value) in [("swapBuyTaxRatio()", 10, buy), ("swapSellTaxRatio()", 11, sell)] {
                self.getter(
                    "APD tax postcondition",
                    sig,
                    &[],
                    77.into(),
                    &e.committed,
                    if ok { value.into() } else { get(&s, slot.into()) },
                    &[slot.into()],
                    save,
                )?;
            }
        }
        let e = self.operation(
            "APD pair setter",
            "setMainPair(address)",
            &[55.into()],
            1.into(),
            &base(t),
            Exit::Return(vec![]),
            &[(7.into(), 55.into())],
            &[],
            save,
        )?;
        self.getter(
            "APD pair postcondition",
            "mainPair()",
            &[],
            77.into(),
            &e.committed,
            55.into(),
            &[7.into()],
            save,
        )?;
        self.operation(
            "APD unauthorized setter",
            "setMainPair(address)",
            &[55.into()],
            77.into(),
            &base(t),
            Exit::Revert(error("Caller is not admin")),
            &[],
            &[],
            save,
        )?;
        Ok(())
    }
    pub fn projections(&mut self, save: &mut impl FnMut(&Value) -> Result<()>) -> Result<Vec<Value>> {
        let mut results = self.raw_projections.clone();
        for m in self.operations.clone() {
            let getter = if matches!(m.execution.exit, Exit::Return(_)) {
                let who = holders(self.target)[0];
                let mut state = m.execution.committed.clone();
                state.insert(balance(who), 987.into());
                Some(self.getter(
                    "mixed synthetic balance getter",
                    "balanceOf(address)",
                    &[who],
                    77.into(),
                    &state,
                    987.into(),
                    &[balance(who)],
                    save,
                )?)
            } else {
                None
            };
            results.push(super::projector::verify_operation(&m, getter.as_ref())?);
        }
        Ok(results)
    }
    pub fn dsg_operations(&mut self, save: &mut impl FnMut(&Value) -> Result<()>) -> Result<()> {
        ensure!(self.target == Target::Dsg, "DSG only");
        let t = self.target;
        let mut s = base(t);
        for amount in [U256::max_value(), U256::zero(), 123.into()] {
            let e = self.operation(
                "DSG approve/reset",
                "approve(address,uint256)",
                &[55.into(), amount],
                44.into(),
                &s,
                Exit::Return(vm::word(1.into()).to_vec()),
                &[(allowance(44.into(), 55.into()), amount)],
                &[(vec![role("Approval(address,address,uint256)"), 44.into(), 55.into()], vm::word(amount).to_vec())],
                save,
            )?;
            self.getter(
                "DSG approved allowance",
                "allowance(address,address)",
                &[44.into(), 55.into()],
                77.into(),
                &e.committed,
                amount,
                &[allowance(44.into(), 55.into())],
                save,
            )?;
            s = e.committed;
        }
        for (caller, spender, msg) in [
            (U256::zero(), 55.into(), "ERC20: approve from the zero address"),
            (44.into(), 0.into(), "ERC20: approve to the zero address"),
        ] {
            self.operation(
                "DSG zero approval",
                "approve(address,uint256)",
                &[spender, 7.into()],
                caller,
                &base(t),
                Exit::Revert(error(msg)),
                &[],
                &[],
                save,
            )?;
        }
        for ratio_type in [0u64, 1, 255] {
            for value in [0u64, 100000, 100001] {
                let ok = value <= 100000;
                let slot = if ratio_type == 0 { 12 } else { 11 };
                let writes = if ok { vec![(slot.into(), value.into())] } else { vec![] };
                let logs = if ok {
                    vec![(
                        vec![role("FeeRatioChanged(uint8,uint256)")],
                        [vm::word(ratio_type.into()), vm::word(value.into())].concat(),
                    )]
                } else {
                    vec![]
                };
                let e = self.operation(
                    "DSG ratio boundary",
                    "setRatio(uint8,uint256)",
                    &[ratio_type.into(), value.into()],
                    1.into(),
                    &base(t),
                    if ok { Exit::Return(vec![]) } else { Exit::Revert(error("Exceeds precision")) },
                    &writes,
                    &logs,
                    save,
                )?;
                let sig = if ratio_type == 0 { "buyFeeRatio()" } else { "sellFeeRatio()" };
                self.getter(
                    "DSG ratio postcondition",
                    sig,
                    &[],
                    77.into(),
                    &e.committed,
                    get(&e.committed, slot.into()),
                    &[slot.into()],
                    save,
                )?;
            }
        }
        for who in [0.into(), 55.into()] {
            let e = self.operation(
                "DSG pair setter",
                "setMainPair(address)",
                &[who],
                1.into(),
                &base(t),
                Exit::Return(vec![]),
                &[(9.into(), who)],
                &[],
                save,
            )?;
            self.getter("DSG pair postcondition", "mainPair()", &[], 77.into(), &e.committed, who, &[9.into()], save)?;
        }
        self.operation(
            "DSG unauthorized setter",
            "setRatio(uint8,uint256)",
            &[0.into(), 0.into()],
            77.into(),
            &base(t),
            Exit::Revert(error("Caller is not admin")),
            &[],
            &[],
            save,
        )?;
        Ok(())
    }
    pub fn dsg_saved(&mut self, save: &mut impl FnMut(&Value) -> Result<()>) -> Result<()> {
        ensure!(self.target == Target::Dsg, "DSG only");
        let v = super::saved_dsg()?;
        ensure!(v["runtime_keccak256"] == self.target.runtime_hash(), "saved runtime");
        for c in v["cases"].as_array().context("cases")? {
            let mut s = State::new();
            for (k, v) in c["initial_words"].as_object().context("initial words")? {
                s.insert(parse(k)?, parse(v.as_str().context("word")?)?);
            }
            for (i, h) in holders(self.target).iter().enumerate() {
                s.insert(balance(*h), U256::from(123 + i));
            }
            let before = s.clone();
            self.balances("balance before saved DSG role", &s, save)?;
            let e = self.run(
                c["name"].as_str().context("name")?,
                "re-executed pinned historical synthetic DSG operation",
                c["synthetic_call"]["method"].as_str().context("method")?,
                &super::bytes(&c["synthetic_call"]["calldata"])?,
                parse(c["synthetic_call"]["caller"].as_str().context("caller")?)?,
                &s,
                save,
            )?;
            ensure!(e.exit == Exit::Return(vec![]), "DSG role exit");
            let stores = c["ordered_sstores"].as_array().context("stores")?;
            ensure!(e.writes.len() == stores.len(), "saved store count");
            for (a, b) in e.writes.iter().zip(stores) {
                ensure!(
                    a.key == parse(b["key"].as_str().context("key")?)?
                        && a.old == parse(b["old"].as_str().context("old")?)?
                        && a.new == parse(b["new"].as_str().context("new")?)?
                        && a.pc == b["pc"].as_u64().context("pc")? as usize,
                    "exact historical ordered store/PC"
                );
            }
            for (k, v) in c["final_words"].as_object().context("final")? {
                s.insert(parse(k)?, parse(v.as_str().context("word")?)?);
            }
            ensure!(normalized(&s) == normalized(&e.committed), "all DSG final cells including balance sentinels");
            let changed = !e.writes.is_empty();
            let sig = c["synthetic_call"]["method"].as_str().unwrap();
            let r = parse(c["synthetic_call"]["role"].as_str().unwrap())?;
            let m = parse(c["synthetic_call"]["member"].as_str().unwrap())?;
            let caller = parse(c["synthetic_call"]["caller"].as_str().unwrap())?;
            let logs = if changed {
                vec![(
                    vec![
                        role(if sig.starts_with("grant") {
                            "RoleGranted(bytes32,address,address)"
                        } else {
                            "RoleRevoked(bytes32,address,address)"
                        }),
                        r,
                        m,
                        caller,
                    ],
                    vec![],
                )]
            } else {
                vec![]
            };
            ensure!(
                e.logs.iter().map(|l| (l.topics.clone(), l.data.clone())).collect::<Vec<_>>() == logs,
                "DSG exact role logs"
            );
            ensure!(
                e.logs.iter().all(|l| e.writes.last().is_none_or(|w| w.step < l.step)),
                "DSG log follows its complete operation stores"
            );
            self.operations.push(Measured {
                target: self.target,
                name: c["name"].as_str().unwrap().into(),
                before,
                execution: e.clone(),
            });
            self.balances("balance after saved DSG role", &e.committed, save)?;
            self.getter(
                "saved DSG role membership",
                "hasRole(bytes32,address)",
                &[r, m],
                77.into(),
                &e.committed,
                u8::from(!get(&e.committed, member(self.target, r, m)).is_zero()).into(),
                &[member(self.target, r, m)],
                save,
            )?;
            let h = head(self.target, r);
            let length = get(&s, h);
            self.getter(
                "saved DSG role count",
                "getRoleMemberCount(bytes32)",
                &[r],
                77.into(),
                &e.committed,
                length,
                &[h],
                save,
            )?;
            for i in 0..length.as_usize() {
                let key = vm::hash(&vm::word(h)) + U256::from(i);
                self.getter(
                    "saved DSG role member",
                    "getRoleMember(bytes32,uint256)",
                    &[r, i.into()],
                    77.into(),
                    &e.committed,
                    get(&s, key),
                    &[h, key],
                    save,
                )?;
            }
        }
        Ok(())
    }
}
pub fn parse(s: &str) -> Result<U256> {
    Ok(U256::from_str_radix(s.trim_start_matches("0x"), 16)?)
}
pub fn validate_witnesses(pre: &State, e: &Execution) -> Result<()> {
    let mut s = pre.clone();
    let mut effects = e
        .reads
        .iter()
        .map(|r| (r.step, false, r.key, r.value, r.value))
        .chain(e.writes.iter().map(|w| (w.step, true, w.key, w.old, w.new)))
        .collect::<Vec<_>>();
    effects.sort_by_key(|x| x.0);
    for (_, write, key, old, new) in effects {
        ensure!(get(&s, key) == old, "storage witness continuity");
        if write {
            s.insert(key, new);
        }
    }
    let success = matches!(e.exit, Exit::Return(_));
    ensure!(
        normalized(&e.committed) == normalized(if success { &s } else { pre }),
        "full final storage/rollback"
    );
    ensure!(e.committed_logs == if success { e.logs.clone() } else { vec![] }, "log commit/rollback");
    for h in &e.keccaks {
        ensure!(vm::hash(&h.input) == h.output, "preimage witness");
    }
    Ok(())
}
pub fn check_operation(pre: &State, e: &Execution, exit: &Exit, writes: &[(U256, U256)], logs: &[(Vec<U256>, Vec<u8>)]) -> Result<()> {
    ensure!(e.exit == *exit, "exact operation exit {:?}, expected {:?}", e.exit, exit);
    let mut s = pre.clone();
    let mut expected = vec![];
    for (key, new) in writes {
        expected.push((*key, get(&s, *key), *new));
        s.insert(*key, *new);
    }
    ensure!(
        e.writes.iter().map(|w| (w.key, w.old, w.new)).collect::<Vec<_>>() == expected,
        "exact ordered stores"
    );
    ensure!(
        e.logs.iter().map(|l| (l.topics.clone(), l.data.clone())).collect::<Vec<_>>() == logs,
        "exact logs"
    );
    ensure!(
        e.logs.iter().all(|l| e.writes.last().is_none_or(|w| w.step < l.step)),
        "source log/store interleaving"
    );
    ensure!(
        normalized(&e.committed) == normalized(if matches!(exit, Exit::Return(_)) { &s } else { pre }),
        "all final cells"
    );
    Ok(())
}
