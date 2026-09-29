//! Independent synthetic expected state for Mai's exact source. No admission.
use super::{account, cap, vm};
use crate::ptoken_proof::cases::{call, execution_json, get, mapping, role, state_json, w};
use anyhow::{ensure, Context, Result};
use primitive_types::U256;
use serde_json::{json, Value};
use vm::{Execution, Exit, State};

pub fn member(r: U256, who: U256) -> U256 {
    mapping(who, mapping(r, 0.into()))
}
pub fn admin(r: U256) -> U256 {
    mapping(r, 0.into()).overflowing_add(1.into()).0
}
pub fn head(r: U256) -> U256 {
    mapping(r, 1.into())
}
pub fn element(r: U256, i: U256) -> U256 {
    vm::hash(&vm::word(head(r))).overflowing_add(i).0
}
pub fn position(r: U256, who: U256) -> U256 {
    mapping(who, head(r).overflowing_add(1.into()).0)
}
pub fn balance(who: U256) -> U256 {
    mapping(who, 2.into())
}
pub fn allowance(owner: U256, spender: U256) -> U256 {
    mapping(spender, mapping(owner, 3.into()))
}
fn normalize(s: &State) -> State {
    s.iter().filter(|(_, v)| !v.is_zero()).map(|(k, v)| (*k, *v)).collect()
}
fn packed(s: &str) -> U256 {
    let mut b = [0; 32];
    b[..s.len()].copy_from_slice(s.as_bytes());
    b[31] = (s.len() * 2) as u8;
    U256::from_big_endian(&b)
}
fn set_role(s: &mut State, r: U256, members: &[U256]) {
    s.insert(head(r), members.len().into());
    for (i, m) in members.iter().enumerate() {
        s.insert(member(r, *m), 1.into());
        s.insert(element(r, i.into()), *m);
        s.insert(position(r, *m), (i + 1).into());
    }
}
fn base() -> State {
    let mut s = State::new();
    set_role(&mut s, 0.into(), &[1.into()]);
    set_role(&mut s, role("MINTER_ROLE"), &[1.into()]);
    set_role(&mut s, role("UNRELATED_ROLE"), &[88.into()]);
    s.insert(balance(44.into()), 123.into());
    s.insert(4.into(), 123.into());
    s.insert(allowance(44.into(), 55.into()), 9.into());
    s.insert(5.into(), packed("Matrix DAO"));
    s.insert(6.into(), packed("MAI"));
    s.insert(U256::max_value(), 77.into());
    s
}
fn error(s: &str) -> Vec<u8> {
    let mut b = call("Error(string)", &[32.into(), s.len().into()]);
    b.extend(s.as_bytes());
    b.resize(4 + (b.len() - 4).div_ceil(32) * 32, 0);
    b
}
fn access_error(c: U256, r: U256) -> Vec<u8> {
    error(&format!(
        "AccessControl: account 0x{} is missing role {}",
        hex::encode(&vm::word(c)[12..]),
        w(r)
    ))
}
fn panic(n: u8) -> Vec<u8> {
    call("Panic(uint256)", &[n.into()])
}
#[derive(Clone)]
struct Effects {
    state: State,
    writes: Vec<(U256, U256, U256)>,
    logs: Vec<(Vec<U256>, Vec<u8>, usize)>,
}
impl Effects {
    fn new(s: &State) -> Self {
        Self {
            state: s.clone(),
            writes: vec![],
            logs: vec![],
        }
    }
    fn put(&mut self, k: U256, v: U256) {
        self.writes.push((k, get(&self.state, k), v));
        self.state.insert(k, v);
    }
    fn log(&mut self, topics: Vec<U256>, data: Vec<u8>) {
        self.logs.push((topics, data, self.writes.len()));
    }
    fn role(&mut self, r: U256, m: U256, c: U256, grant: bool) {
        let old = get(&self.state, member(r, m));
        let has = !(old & U256::from(255u64)).is_zero();
        if has != grant {
            self.put(member(r, m), (old & !U256::from(255u64)) | U256::from(u8::from(grant)));
            self.log(
                vec![
                    role(if grant {
                        "RoleGranted(bytes32,address,address)"
                    } else {
                        "RoleRevoked(bytes32,address,address)"
                    }),
                    r,
                    m,
                    c,
                ],
                vec![],
            );
        }
    }
    fn append(&mut self, r: U256, m: U256) -> Result<()> {
        let len = get(&self.state, head(r));
        let next = len.overflowing_add(1.into()).0;
        self.put(head(r), next);
        self.put(element(r, len), m);
        self.put(position(r, m), next);
        Ok(())
    }
    fn remove(&mut self, r: U256, m: U256) -> Result<()> {
        let pos = get(&self.state, position(r, m));
        let len = get(&self.state, head(r));
        let i = pos.checked_sub(1.into()).context("positive position")?;
        let last = len.checked_sub(1.into()).context("positive length")?;
        if i != last {
            let tail = get(&self.state, element(r, last));
            self.put(element(r, i), tail);
            self.put(position(r, tail), pos);
        }
        self.put(element(r, last), 0.into());
        self.put(head(r), last);
        self.put(position(r, m), 0.into());
        Ok(())
    }
}
pub struct Proof<'a> {
    pub runtime: &'a [u8],
    pub calls: usize,
    pub cases: Vec<Value>,
}
impl<'a> Proof<'a> {
    pub fn new(runtime: &'a [u8]) -> Self {
        Self {
            runtime,
            calls: 0,
            cases: vec![],
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn record(
        &mut self,
        name: &str,
        sig: &str,
        code: &[u8],
        kind: &str,
        data: &[u8],
        caller: U256,
        pre: &State,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<Execution> {
        let e = vm::execute(code, data, caller, account(), pre);
        self.calls += 1;
        let v = json!({"name":name,"signature":sig,"scope":"synthetic source operation; not coherent-state admission","code_kind":kind,"code_sha256":super::sha(code),"caller":w(caller),"address":w(account()),"calldata":hex::encode(data),"prestate":state_json(pre),"execution":execution_json(&e)});
        save(self.calls, &v)?;
        self.cases.push(v);
        ensure!(
            !matches!(e.exit, Exit::Invalid | Exit::HarnessFailure(_)),
            "{name}: unexpected exit {:?}",
            e.exit
        );
        for h in &e.keccaks {
            ensure!(vm::hash(&h.input) == h.output, "{name}: Keccak witness");
        }
        let mut state = pre.clone();
        let mut writes = e.writes.iter().peekable();
        for read in &e.reads {
            while writes.peek().is_some_and(|w| w.step < read.step) {
                let w = writes.next().unwrap();
                ensure!(get(&state, w.key) == w.old, "write continuity");
                state.insert(w.key, w.new);
            }
            ensure!(get(&state, read.key) == read.value, "{name}: read continuity");
        }
        Ok(e)
    }
    fn run(
        &mut self,
        name: &str,
        sig: &str,
        args: &[U256],
        caller: U256,
        pre: &State,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<Execution> {
        self.record(name, sig, self.runtime, "deployedBytecode", &call(sig, args), caller, pre, save)
    }
    fn check(name: &str, e: &Execution, pre: &State, want: &Effects, exit: Exit) -> Result<()> {
        ensure!(e.exit == exit, "{name}: exact exit actual={:?} expected={exit:?}", e.exit);
        ensure!(
            e.writes.iter().map(|v| (v.key, v.old, v.new)).collect::<Vec<_>>() == want.writes,
            "{name}: exact ordered stores actual={:?} expected={:?}",
            e.writes.iter().map(|v| (v.key, v.old, v.new)).collect::<Vec<_>>(),
            want.writes
        );
        ensure!(
            e.logs.iter().map(|l| (l.topics.clone(), l.data.clone())).collect::<Vec<_>>()
                == want.logs.iter().map(|(t, d, _)| (t.clone(), d.clone())).collect::<Vec<_>>(),
            "{name}: exact logs"
        );
        for (log, (_, _, after)) in e.logs.iter().zip(&want.logs) {
            if *after > 0 {
                ensure!(e.writes[after - 1].step < log.step, "log follows store");
            }
            if *after < e.writes.len() {
                ensure!(log.step < e.writes[*after].step, "log precedes store");
            }
        }
        if matches!(exit, Exit::Return(_)) {
            ensure!(
                normalize(&e.committed) == normalize(&want.state) && e.committed_logs == e.logs,
                "{name}: all-cell committed state/logs"
            );
        } else {
            ensure!(e.committed == *pre && e.committed_logs.is_empty(), "{name}: exact rollback");
        }
        Ok(())
    }
    fn getter(&mut self, name: &str, sig: &str, args: &[U256], data: Vec<u8>, pre: &State, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let e = self.run(name, sig, args, 9.into(), pre, save)?;
        Self::check(name, &e, pre, &Effects::new(pre), Exit::Return(data))
    }
    fn scalar(&mut self, name: &str, sig: &str, args: &[U256], want: U256, pre: &State, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        self.getter(name, sig, args, vm::word(want).to_vec(), pre, save)
    }
    fn verify_set(
        &mut self,
        name: &str,
        r: U256,
        members: &[U256],
        checked: U256,
        s: &State,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<()> {
        self.scalar(name, "getRoleMemberCount(bytes32)", &[r], members.len().into(), s, save)?;
        self.scalar(name, "getRoleAdmin(bytes32)", &[r], get(s, admin(r)), s, save)?;
        for (i, m) in members.iter().enumerate() {
            self.scalar(name, "getRoleMember(bytes32,uint256)", &[r, i.into()], *m, s, save)?;
            ensure!(get(s, position(r, *m)) == (i + 1).into(), "one-based coherence");
        }
        for m in members
            .iter()
            .copied()
            .chain([checked, 0.into(), 99.into()])
            .collect::<std::collections::BTreeSet<_>>()
        {
            self.scalar(name, "hasRole(bytes32,address)", &[r, m], u8::from(members.contains(&m)).into(), s, save)?;
        }
        let e = self.run(name, "getRoleMember(bytes32,uint256)", &[r, members.len().into()], 9.into(), s, save)?;
        Self::check(name, &e, s, &Effects::new(s), Exit::Revert(panic(0x32)))
    }
    #[allow(clippy::too_many_arguments)]
    fn operation(
        &mut self,
        name: &str,
        r: U256,
        before: &[U256],
        m: U256,
        grant: bool,
        renounce: bool,
        previous: Option<State>,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<State> {
        let mut pre = previous.unwrap_or_else(base);
        set_role(&mut pre, r, before);
        let c = if renounce { m } else { 1.into() };
        let sig = if grant {
            "grantRole(bytes32,address)"
        } else if renounce {
            "renounceRole(bytes32,address)"
        } else {
            "revokeRole(bytes32,address)"
        };
        let mut want = Effects::new(&pre);
        want.role(r, m, c, grant);
        let mut after = before.to_vec();
        let ix = before.iter().position(|v| *v == m);
        if grant && ix.is_none() {
            want.append(r, m)?;
            after.push(m);
        } else if !grant && ix.is_some() {
            want.remove(r, m)?;
            after.swap_remove(ix.unwrap());
        }
        let e = self.run(name, sig, &[r, m], c, &pre, save)?;
        Self::check(name, &e, &pre, &want, Exit::Return(vec![]))?;
        for key in [member(r, m), position(r, m)] {
            ensure!(e.reads.iter().any(|v| v.key == key), "{name}: bool and index read even for noops");
        }
        if !renounce {
            ensure!(
                e.reads.iter().any(|v| v.key == admin(r)) && e.reads.iter().any(|v| v.key == member(get(&pre, admin(r)), c)),
                "exact authority reads"
            );
        }
        for preimage in [
            [vm::word(r), vm::word(0.into())].concat(),
            [vm::word(r), vm::word(1.into())].concat(),
            [vm::word(m), vm::word(head(r).overflowing_add(1.into()).0)].concat(),
        ] {
            ensure!(e.keccaks.iter().any(|v| v.input == preimage), "root/member witness");
        }
        self.verify_set(name, r, &after, m, &e.committed, save)?;
        for (sig, args, value) in [
            ("balanceOf(address)", vec![44.into()], 123),
            ("totalSupply()", vec![], 123),
            ("allowance(address,address)", vec![44.into(), 55.into()], 9),
        ] {
            self.scalar(name, sig, &args, U256::from(value as u64), &e.committed, save)?;
        }
        Ok(e.committed)
    }
    pub fn constructor(&mut self, capture: &Value, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let code = super::bytes(&capture["creationBytecode"]["onchainBytecode"])?;
        let caller = U256::from_str_radix("512Be7Ce91b9BC5D9190e4D6F273f48Db33eB2F4", 16)?;
        let pre = State::new();
        let mut want = Effects::new(&pre);
        want.put(5.into(), packed("Matrix DAO"));
        want.put(6.into(), packed("MAI"));
        for r in [U256::zero(), role("MINTER_ROLE")] {
            want.role(r, caller, caller, true);
            want.append(r, caller)?;
        }
        let e = self.record("captured_no_argument_constructor", "constructor()", &code, "bytecode", &[], caller, &pre, save)?;
        Self::check("constructor", &e, &pre, &want, Exit::Return(self.runtime.to_vec()))?;
        for r in [U256::zero(), role("MINTER_ROLE")] {
            self.verify_set("constructor_role", r, &[caller], 0.into(), &e.committed, save)?;
        }
        self.scalar("constructor_cap", "cap()", &[], cap(), &e.committed, save)?;
        self.scalar("constructor_supply", "totalSupply()", &[], 0.into(), &e.committed, save)?;
        Ok(())
    }
    pub fn matrix(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let r = role("MATRIX_ROLE");
        for (name, values, m, g) in [
            ("grant_empty", vec![], 2, true),
            ("grant_nonempty", vec![2, 3], 4, true),
            ("grant_zero", vec![2, 3], 0, true),
            ("grant_only_zero", vec![], 0, true),
            ("duplicate", vec![2, 3], 2, true),
            ("duplicate_zero", vec![0, 3], 0, true),
            ("remove_first", vec![2, 3, 4], 2, false),
            ("remove_middle", vec![2, 3, 4], 3, false),
            ("remove_tail", vec![2, 3, 4], 4, false),
            ("remove_only", vec![2], 2, false),
            ("remove_zero", vec![0, 3], 0, false),
            ("remove_zero_tail", vec![2, 0], 0, false),
            ("remove_only_zero", vec![0], 0, false),
            ("move_zero_tail", vec![2, 3, 0], 3, false),
            ("absent_empty", vec![], 2, false),
            ("absent_nonempty", vec![2, 3], 4, false),
        ] {
            self.operation(
                name,
                r,
                &values.into_iter().map(|v| U256::from(v as u64)).collect::<Vec<_>>(),
                U256::from(m as u64),
                g,
                false,
                None,
                save,
            )?;
        }
        for r in [U256::zero(), role("MINTER_ROLE"), U256::max_value(), U256::one() << 255] {
            let before = if r.is_zero() || r == role("MINTER_ROLE") { vec![1.into()] } else { vec![] };
            self.operation("full_role_identity", r, &before, 2.into(), true, false, None, save)?;
        }
        self.operation("renounce_present", r, &[2.into(), 3.into()], 2.into(), false, true, None, save)?;
        self.operation("renounce_absent", r, &[3.into()], 2.into(), false, true, None, save)?;
        let mut state = base();
        let mut members = vec![];
        for (i, (g, m)) in [(true, 2u64), (true, 0), (true, 3), (false, 2), (true, 2), (false, 0), (false, 2), (false, 3)]
            .into_iter()
            .enumerate()
        {
            state = self.operation(&format!("sequence_{i}"), r, &members, m.into(), g, false, Some(state), save)?;
            if g {
                members.push(m.into());
            } else {
                members.swap_remove(members.iter().position(|v| *v == m.into()).unwrap());
            }
        }
        self.authority_and_abi(save)?;
        self.incoherent(save)?;
        self.malformed_boundaries(save)?;
        self.token_matrix(save)?;
        self.raw_getters(save)
    }
    fn authority_and_abi(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let r = role("AUTH");
        let pre = base();
        for sig in ["grantRole(bytes32,address)", "revokeRole(bytes32,address)"] {
            let e = self.run("unauthorized", sig, &[r, 2.into()], 9.into(), &pre, save)?;
            Self::check("unauthorized", &e, &pre, &Effects::new(&pre), Exit::Revert(access_error(9.into(), 0.into())))?;
        }
        let e = self.run("wrong_renounce", "renounceRole(bytes32,address)", &[r, 2.into()], 9.into(), &pre, save)?;
        Self::check(
            "wrong_renounce",
            &e,
            &pre,
            &Effects::new(&pre),
            Exit::Revert(error("AccessControl: can only renounce roles for self")),
        )?;
        let mut pre = base();
        pre.insert(admin(r), U256::max_value());
        set_role(&mut pre, U256::max_value(), &[9.into()]);
        for caller in [1u64, 9] {
            let mut want = Effects::new(&pre);
            let exit = if caller == 9 {
                want.role(r, 2.into(), 9.into(), true);
                want.append(r, 2.into())?;
                Exit::Return(vec![])
            } else {
                Exit::Revert(access_error(1.into(), U256::max_value()))
            };
            let e = self.run(
                "custom_full_width_admin",
                "grantRole(bytes32,address)",
                &[r, 2.into()],
                caller.into(),
                &pre,
                save,
            )?;
            Self::check("custom_admin", &e, &pre, &want, exit)?;
        }
        let mut want = Effects::new(&base());
        want.role(0.into(), 1.into(), 1.into(), false);
        want.remove(0.into(), 1.into())?;
        let e = self.run(
            "last_admin_loss",
            "renounceRole(bytes32,address)",
            &[0.into(), 1.into()],
            1.into(),
            &base(),
            save,
        )?;
        Self::check("last_admin_loss", &e, &base(), &want, Exit::Return(vec![]))?;
        let pre = e.committed;
        let e = self.run(
            "last_admin_cannot_regain",
            "grantRole(bytes32,address)",
            &[0.into(), 1.into()],
            1.into(),
            &pre,
            save,
        )?;
        Self::check(
            "last_admin_cannot_regain",
            &e,
            &pre,
            &Effects::new(&pre),
            Exit::Revert(access_error(1.into(), 0.into())),
        )?;
        for (name, data) in [
            ("unknown_selector", vec![0xff; 4]),
            ("empty_calldata", vec![]),
            ("truncated_calldata", call("grantRole(bytes32,address)", &[r])),
            ("dirty_address", call("grantRole(bytes32,address)", &[r, U256::one() << 160])),
        ] {
            let pre = base();
            let e = self.record(name, "raw", self.runtime, "deployedBytecode", &data, 1.into(), &pre, save)?;
            Self::check(name, &e, &pre, &Effects::new(&pre), Exit::Revert(vec![]))?;
        }
        for (signature, args, address_indexes) in [
            ("balanceOf(address)", vec![2.into()], vec![0]),
            ("allowance(address,address)", vec![2.into(), 4.into()], vec![0, 1]),
            ("mint(address,uint256)", vec![2.into(), 0.into()], vec![0]),
            ("burnFrom(address,uint256)", vec![2.into(), 0.into()], vec![0]),
            ("transfer(address,uint256)", vec![2.into(), 0.into()], vec![0]),
            ("transferFrom(address,address,uint256)", vec![2.into(), 3.into(), 0.into()], vec![0, 1]),
            ("approve(address,uint256)", vec![2.into(), 0.into()], vec![0]),
            ("increaseAllowance(address,uint256)", vec![2.into(), 0.into()], vec![0]),
            ("decreaseAllowance(address,uint256)", vec![2.into(), 0.into()], vec![0]),
            ("hasRole(bytes32,address)", vec![r, 2.into()], vec![1]),
            ("revokeRole(bytes32,address)", vec![r, 2.into()], vec![1]),
            ("renounceRole(bytes32,address)", vec![r, 2.into()], vec![1]),
            ("getRoleMember(bytes32,uint256)", vec![r, 0.into()], vec![]),
            ("getRoleMemberCount(bytes32)", vec![r], vec![]),
            ("getRoleAdmin(bytes32)", vec![r], vec![]),
            ("burn(uint256)", vec![0.into()], vec![]),
        ] {
            let pre = base();
            let mut truncated = call(signature, &args);
            truncated.pop();
            let e = self.record(
                "abi_truncated_entrypoint",
                signature,
                self.runtime,
                "deployedBytecode",
                &truncated,
                1.into(),
                &pre,
                save,
            )?;
            Self::check("abi_truncated_entrypoint", &e, &pre, &Effects::new(&pre), Exit::Revert(vec![]))?;
            for i in address_indexes {
                let mut dirty = args.clone();
                dirty[i] = U256::one() << 160;
                let e = self.run("abi_dirty_entrypoint", signature, &dirty, 1.into(), &pre, save)?;
                Self::check("abi_dirty_entrypoint", &e, &pre, &Effects::new(&pre), Exit::Revert(vec![]))?;
            }
        }
        let pre = base();
        let e = self.run("dirty_bytes4", "supportsInterface(bytes4)", &[1.into()], 1.into(), &pre, save)?;
        Self::check("dirty_bytes4", &e, &pre, &Effects::new(&pre), Exit::Revert(vec![]))?;
        let pre = base();
        let mut data = call("grantRole(bytes32,address)", &[r, 2.into()]);
        data.extend([0xaa; 7]);
        let mut want = Effects::new(&pre);
        want.role(r, 2.into(), 1.into(), true);
        want.append(r, 2.into())?;
        let e = self.record("trailing_calldata", "raw", self.runtime, "deployedBytecode", &data, 1.into(), &pre, save)?;
        Self::check("trailing_calldata", &e, &pre, &want, Exit::Return(vec![]))?;
        Ok(())
    }
    fn incoherent(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let r = role("INCOHERENT");
        let m = U256::from(2u64);
        for (name, g, bool_present, set_present) in [
            ("grant_bool_only", true, false, true),
            ("grant_set_only", true, true, false),
            ("revoke_set_only", false, false, true),
            ("revoke_bool_only", false, true, false),
        ] {
            let mut pre = base();
            if set_present {
                set_role(&mut pre, r, &[m]);
            }
            pre.insert(member(r, m), u8::from(bool_present).into());
            let mut want = Effects::new(&pre);
            want.role(r, m, 1.into(), g);
            if g && !set_present {
                want.append(r, m)?;
            } else if !g && set_present {
                want.remove(r, m)?;
            }
            let e = self.run(
                name,
                if g { "grantRole(bytes32,address)" } else { "revokeRole(bytes32,address)" },
                &[r, m],
                1.into(),
                &pre,
                save,
            )?;
            Self::check(name, &e, &pre, &want, Exit::Return(vec![]))?;
        }
        for old in [U256::one() << 8, (U256::one() << 255) | U256::from(2u64)] {
            let mut pre = base();
            pre.insert(member(r, m), old);
            let mut want = Effects::new(&pre);
            want.role(r, m, 1.into(), true);
            want.append(r, m)?;
            let e = self.run("packed_boolean_padding", "grantRole(bytes32,address)", &[r, m], 1.into(), &pre, save)?;
            Self::check("packed_boolean_padding", &e, &pre, &want, Exit::Return(vec![]))?;
        }
        for (name, len, pos, expect) in [("empty_length_position", 0u64, 1u64, 0x11u8), ("position_past_length", 1, 2, 0x32)] {
            let mut pre = base();
            pre.insert(member(r, m), 1.into());
            pre.insert(head(r), len.into());
            pre.insert(position(r, m), pos.into());
            pre.insert(element(r, 0.into()), m);
            let mut want = Effects::new(&pre);
            want.role(r, m, 1.into(), false);
            let e = self.run(name, "revokeRole(bytes32,address)", &[r, m], 1.into(), &pre, save)?;
            Self::check(name, &e, &pre, &want, Exit::Revert(panic(expect)))?;
        }
        Ok(())
    }
    fn malformed_boundaries(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let r = role("MALFORMED_BOUNDARY");
        let m = U256::from(2u64);
        for len in [(U256::one() << 64) - 1, U256::one() << 64, U256::max_value() - 1, U256::max_value()] {
            let mut pre = base();
            pre.insert(head(r), len);
            pre.insert(element(r, len), 77.into());
            let mut want = Effects::new(&pre);
            want.role(r, m, 1.into(), true);
            want.append(r, m)?;
            let e = self.run(
                "malformed_length_push_source_semantics",
                "grantRole(bytes32,address)",
                &[r, m],
                1.into(),
                &pre,
                save,
            )?;
            Self::check("malformed_length_push_source_semantics", &e, &pre, &want, Exit::Return(vec![]))?;
        }
        let dirty = (U256::one() << 200) | U256::from(3u64);
        let mut pre = base();
        set_role(&mut pre, r, &[m, 3.into()]);
        pre.insert(element(r, 1.into()), dirty);
        self.scalar(
            "dirty_array_getter_mask",
            "getRoleMember(bytes32,uint256)",
            &[r, 1.into()],
            3.into(),
            &pre,
            save,
        )?;
        let mut want = Effects::new(&pre);
        want.role(r, m, 1.into(), false);
        want.remove(r, m)?;
        let e = self.run(
            "dirty_tail_moves_full_bytes32_index",
            "revokeRole(bytes32,address)",
            &[r, m],
            1.into(),
            &pre,
            save,
        )?;
        Self::check("dirty_tail_moves_full_bytes32_index", &e, &pre, &want, Exit::Return(vec![]))?;
        Ok(())
    }
    fn raw_getters(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let r = U256::max_value();
        let m = (U256::one() << 160) - 1;
        for value in [U256::zero(), 1.into(), U256::one() << 255, U256::max_value()] {
            let mut s = base();
            s.insert(balance(m), value);
            s.insert(allowance(m, 2.into()), value);
            s.insert(4.into(), value);
            s.insert(admin(r), value);
            s.insert(head(r), value);
            s.insert(member(r, m), value);
            for (sig, args, want) in [
                ("balanceOf(address)", vec![m], value),
                ("allowance(address,address)", vec![m, 2.into()], value),
                ("totalSupply()", vec![], value),
                ("getRoleAdmin(bytes32)", vec![r], value),
                ("getRoleMemberCount(bytes32)", vec![r], value),
                ("hasRole(bytes32,address)", vec![r, m], u8::from(!(value & U256::from(255)).is_zero()).into()),
                ("cap()", vec![], cap()),
                ("decimals()", vec![], 18.into()),
                ("MINTER_ROLE()", vec![], role("MINTER_ROLE")),
                ("DEFAULT_ADMIN_ROLE()", vec![], 0.into()),
            ] {
                self.scalar("raw_word_getters", sig, &args, want, &s, save)?;
            }
        }
        for (sig, s) in [("name()", "Matrix DAO"), ("symbol()", "MAI")] {
            let mut data = vm::word(32.into()).to_vec();
            data.extend(vm::word(s.len().into()));
            data.extend(s.as_bytes());
            data.resize(data.len().div_ceil(32) * 32, 0);
            self.getter("metadata", sig, &[], data, &base(), save)?;
        }
        for (id, ok) in [(0x01ffc9a7u64, true), (0x7965db0b, true), (0x5a05180f, true), (0xffffffff, false)] {
            self.scalar(
                "interfaces",
                "supportsInterface(bytes4)",
                &[U256::from(id) << 224],
                u8::from(ok).into(),
                &base(),
                save,
            )?;
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn token(&mut self, name: &str, sig: &str, args: &[U256], caller: U256, pre: &State, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<State> {
        let mut want = Effects::new(pre);
        fn approve(w: &mut Effects, owner: U256, spender: U256, amount: U256) -> std::result::Result<(), Vec<u8>> {
            if owner.is_zero() {
                return Err(error("ERC20: approve from the zero address"));
            }
            if spender.is_zero() {
                return Err(error("ERC20: approve to the zero address"));
            }
            w.put(allowance(owner, spender), amount);
            w.log(vec![role("Approval(address,address,uint256)"), owner, spender], vm::word(amount).to_vec());
            Ok(())
        }
        fn spend(w: &mut Effects, owner: U256, spender: U256, amount: U256) -> std::result::Result<(), Vec<u8>> {
            let current = get(&w.state, allowance(owner, spender));
            if current != U256::max_value() {
                if current < amount {
                    return Err(error("ERC20: insufficient allowance"));
                }
                approve(w, owner, spender, current - amount)?;
            }
            Ok(())
        }
        let result = (|| -> std::result::Result<Vec<u8>, Vec<u8>> {
            match sig {
                "mint(address,uint256)" => {
                    let to = args[0];
                    let amount = args[1];
                    if (get(pre, member(role("MINTER_ROLE"), caller)) & U256::from(255)).is_zero() {
                        return Err(error("ERC20: Must have minter role to mint"));
                    }
                    let supply = get(pre, 4.into()).checked_add(amount).ok_or_else(|| panic(0x11))?;
                    if supply > cap() {
                        return Err(error("ERC20Capped: cap exceeded"));
                    }
                    if to.is_zero() {
                        return Err(error("ERC20: mint to the zero address"));
                    }
                    want.put(4.into(), supply);
                    let b = get(&want.state, balance(to)).checked_add(amount).ok_or_else(|| panic(0x11))?;
                    want.put(balance(to), b);
                    want.log(vec![role("Transfer(address,address,uint256)"), 0.into(), to], vm::word(amount).to_vec());
                    Ok(vec![])
                }
                "burn(uint256)" | "burnFrom(address,uint256)" => {
                    let (from, amount) = if sig == "burn(uint256)" { (caller, args[0]) } else { (args[0], args[1]) };
                    if sig.starts_with("burnFrom") {
                        spend(&mut want, from, caller, amount)?;
                    }
                    if from.is_zero() {
                        return Err(error("ERC20: burn from the zero address"));
                    }
                    let b = get(&want.state, balance(from));
                    if b < amount {
                        return Err(error("ERC20: burn amount exceeds balance"));
                    }
                    want.put(balance(from), b - amount);
                    let supply = get(&want.state, 4.into()).checked_sub(amount).ok_or_else(|| panic(0x11))?;
                    want.put(4.into(), supply);
                    want.log(vec![role("Transfer(address,address,uint256)"), from, 0.into()], vm::word(amount).to_vec());
                    Ok(vec![])
                }
                "transfer(address,uint256)" | "transferFrom(address,address,uint256)" => {
                    let (from, to, amount) = if sig == "transfer(address,uint256)" {
                        (caller, args[0], args[1])
                    } else {
                        (args[0], args[1], args[2])
                    };
                    if sig.starts_with("transferFrom") {
                        spend(&mut want, from, caller, amount)?;
                    }
                    if from.is_zero() {
                        return Err(error("ERC20: transfer from the zero address"));
                    }
                    if to.is_zero() {
                        return Err(error("ERC20: transfer to the zero address"));
                    }
                    let b = get(&want.state, balance(from));
                    if b < amount {
                        return Err(error("ERC20: transfer amount exceeds balance"));
                    }
                    want.put(balance(from), b - amount);
                    let credit = get(&want.state, balance(to)).checked_add(amount).ok_or_else(|| panic(0x11))?;
                    want.put(balance(to), credit);
                    want.log(vec![role("Transfer(address,address,uint256)"), from, to], vm::word(amount).to_vec());
                    Ok(vm::word(1.into()).to_vec())
                }
                "approve(address,uint256)" => {
                    approve(&mut want, caller, args[0], args[1])?;
                    Ok(vm::word(1.into()).to_vec())
                }
                "increaseAllowance(address,uint256)" => {
                    let amount = get(pre, allowance(caller, args[0])).checked_add(args[1]).ok_or_else(|| panic(0x11))?;
                    approve(&mut want, caller, args[0], amount)?;
                    Ok(vm::word(1.into()).to_vec())
                }
                "decreaseAllowance(address,uint256)" => {
                    let old = get(pre, allowance(caller, args[0]));
                    if old < args[1] {
                        return Err(error("ERC20: decreased allowance below zero"));
                    }
                    approve(&mut want, caller, args[0], old - args[1])?;
                    Ok(vm::word(1.into()).to_vec())
                }
                _ => unreachable!(),
            }
        })();
        let exit = match result {
            Ok(data) => Exit::Return(data),
            Err(data) => Exit::Revert(data),
        };
        let e = self.run(name, sig, args, caller, pre, save)?;
        Self::check(name, &e, pre, &want, exit)?;
        self.scalar(name, "cap()", &[], cap(), &e.committed, save)?;
        self.scalar(name, "totalSupply()", &[], get(&e.committed, 4.into()), &e.committed, save)?;
        for m in [U256::from(2u64), 3.into(), 44.into()] {
            self.scalar(name, "balanceOf(address)", &[m], get(&e.committed, balance(m)), &e.committed, save)?;
        }
        Ok(e.committed)
    }
    fn token_matrix(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let mut normal = base();
        normal.insert(balance(2.into()), 100.into());
        normal.insert(4.into(), 223.into());
        normal.insert(allowance(2.into(), 4.into()), 30.into());
        let mut sequence = normal.clone();
        for (name, sig, args, c) in [
            ("mint_sequence", "mint(address,uint256)", vec![2.into(), 100.into()], 1u64),
            ("transfer_sequence", "transfer(address,uint256)", vec![3.into(), 25.into()], 2),
            ("burn_sequence", "burn(uint256)", vec![10.into()], 2),
            ("approve_sequence", "approve(address,uint256)", vec![4.into(), 20.into()], 2),
            (
                "transfer_from_sequence",
                "transferFrom(address,address,uint256)",
                vec![2.into(), 3.into(), 5.into()],
                4,
            ),
            ("burn_from_sequence", "burnFrom(address,uint256)", vec![2.into(), 5.into()], 4),
        ] {
            sequence = self.token(name, sig, &args, c.into(), &sequence, save)?;
        }
        for amount in [U256::zero(), 1.into(), 100.into(), 101.into(), U256::max_value()] {
            for to in [U256::zero(), 2.into(), 3.into()] {
                self.token("transfer_boundaries", "transfer(address,uint256)", &[to, amount], 2.into(), &normal, save)?;
            }
            self.token("burn_boundaries", "burn(uint256)", &[amount], 2.into(), &normal, save)?;
            self.token("mint_authority", "mint(address,uint256)", &[3.into(), amount], 9.into(), &normal, save)?;
            for current in [U256::zero(), 30.into(), U256::max_value()] {
                let mut s = normal.clone();
                s.insert(allowance(2.into(), 4.into()), current);
                for to in [U256::zero(), 2.into(), 3.into()] {
                    self.token(
                        "transfer_from_allowance",
                        "transferFrom(address,address,uint256)",
                        &[2.into(), to, amount],
                        4.into(),
                        &s,
                        save,
                    )?;
                }
                self.token("burn_from_allowance", "burnFrom(address,uint256)", &[2.into(), amount], 4.into(), &s, save)?;
            }
        }
        for supply in [U256::zero(), cap() - 1, cap(), U256::max_value()] {
            for amount in [U256::zero(), 1.into(), cap(), U256::max_value()] {
                for to in [U256::zero(), 2.into()] {
                    let mut s = normal.clone();
                    s.insert(4.into(), supply);
                    self.token("mint_cap_order", "mint(address,uint256)", &[to, amount], 1.into(), &s, save)?;
                }
            }
        }
        for sig in [
            "approve(address,uint256)",
            "increaseAllowance(address,uint256)",
            "decreaseAllowance(address,uint256)",
        ] {
            for current in [U256::zero(), 3.into(), U256::max_value()] {
                for amount in [U256::zero(), 1.into(), U256::max_value()] {
                    for spender in [U256::zero(), 4.into()] {
                        let mut s = normal.clone();
                        s.insert(allowance(2.into(), spender), current);
                        self.token("allowance_arithmetic_order", sig, &[spender, amount], 2.into(), &s, save)?;
                    }
                }
            }
        }
        let mut late = normal.clone();
        late.insert(balance(3.into()), U256::max_value());
        self.token(
            "transfer_credit_overflow",
            "transfer(address,uint256)",
            &[3.into(), 1.into()],
            2.into(),
            &late,
            save,
        )?;
        self.token(
            "transfer_from_late_overflow",
            "transferFrom(address,address,uint256)",
            &[2.into(), 3.into(), 1.into()],
            4.into(),
            &late,
            save,
        )?;
        self.token("mint_credit_overflow", "mint(address,uint256)", &[3.into(), 1.into()], 1.into(), &late, save)?;
        let mut late = normal.clone();
        late.insert(4.into(), 0.into());
        self.token("burn_supply_underflow", "burn(uint256)", &[1.into()], 2.into(), &late, save)?;
        self.token(
            "burn_from_supply_underflow",
            "burnFrom(address,uint256)",
            &[2.into(), 1.into()],
            4.into(),
            &late,
            save,
        )?;
        for sig in [
            "approve(address,uint256)",
            "increaseAllowance(address,uint256)",
            "decreaseAllowance(address,uint256)",
            "transfer(address,uint256)",
            "burn(uint256)",
        ] {
            let args = if sig == "burn(uint256)" { vec![0.into()] } else { vec![3.into(), 0.into()] };
            self.token("zero_sender", sig, &args, 0.into(), &normal, save)?;
        }
        for sig in ["transferFrom(address,address,uint256)", "burnFrom(address,uint256)"] {
            for infinite in [false, true] {
                let mut s = normal.clone();
                s.insert(allowance(0.into(), 4.into()), if infinite { U256::max_value() } else { 0.into() });
                let args = if sig.starts_with("transferFrom") {
                    vec![0.into(), 3.into(), 0.into()]
                } else {
                    vec![0.into(), 0.into()]
                };
                self.token("zero_owner_allowance_order", sig, &args, 4.into(), &s, save)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_log_wrong_store_and_bad_committed_state_return_errors_without_panics() {
        let c = super::super::verify_capture(super::super::captured()).unwrap();
        let runtime = super::super::runtime(&c).unwrap();
        let r = role("ROBUSTNESS");
        let pre = base();
        let mut want = Effects::new(&pre);
        want.role(r, 2.into(), 1.into(), true);
        want.append(r, 2.into()).unwrap();
        let e = vm::execute(&runtime, &call("grantRole(bytes32,address)", &[r, 2.into()]), 1.into(), account(), &pre);
        Proof::check("control", &e, &pre, &want, Exit::Return(vec![])).unwrap();
        let mut no_log = e.clone();
        no_log.logs.clear();
        assert!(Proof::check("missing_log", &no_log, &pre, &want, Exit::Return(vec![])).is_err());
        let mut wrong_store = e.clone();
        wrong_store.writes[0].new = 0.into();
        assert!(Proof::check("wrong_store", &wrong_store, &pre, &want, Exit::Return(vec![])).is_err());
        let mut extra = e;
        extra.committed.insert(123456.into(), 1.into());
        assert!(Proof::check("extra_cell", &extra, &pre, &want, Exit::Return(vec![])).is_err());
    }
}
