//! Separate captured WKEYDAO/GOT bytecode in synthetic local state, never admission.
use super::{address, bytes, vm, Target};
use crate::ptoken_proof::cases::{call, execution_json, get, mapping, role, state_json, w};
use anyhow::{ensure, Context, Result};
use primitive_types::U256;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use vm::{Execution, Exit, State};

pub fn head(t: Target, r: U256) -> U256 {
    mapping(r, t.role_root())
}
pub fn admin(t: Target, r: U256) -> U256 {
    head(t, r).overflowing_add(2.into()).0
}
pub fn element(t: Target, r: U256, i: U256) -> U256 {
    vm::hash(&vm::word(head(t, r))).overflowing_add(i).0
}
pub fn position(t: Target, r: U256, m: U256) -> U256 {
    mapping(m, head(t, r).overflowing_add(1.into()).0)
}
fn normalized(s: &State) -> State {
    s.iter().filter(|(_, v)| !v.is_zero()).map(|(k, v)| (*k, *v)).collect()
}
fn packed(s: &str) -> U256 {
    assert!(s.len() < 32);
    let mut b = [0; 32];
    b[..s.len()].copy_from_slice(s.as_bytes());
    b[31] = (2 * s.len()) as u8;
    U256::from_big_endian(&b)
}
fn words(v: &[U256]) -> Vec<u8> {
    v.iter().flat_map(|n| vm::word(*n)).collect()
}
fn string_tail(s: &str) -> Vec<u8> {
    let mut v = vm::word(s.len().into()).to_vec();
    v.extend(s.as_bytes());
    v.resize(v.len().div_ceil(32) * 32, 0);
    v
}
fn error(s: &str) -> Vec<u8> {
    [call("Error(string)", &[32.into()]), string_tail(s)].concat()
}
fn seed(t: Target, s: &mut State, r: U256, members: &[U256]) {
    assert!(get(s, head(t, r)) < 32.into());
    for i in 0..get(s, head(t, r)).as_usize() {
        let m = get(s, element(t, r, i.into()));
        s.remove(&element(t, r, i.into()));
        s.remove(&position(t, r, m));
    }
    s.insert(head(t, r), members.len().into());
    for (i, m) in members.iter().enumerate() {
        s.insert(element(t, r, i.into()), *m);
        s.insert(position(t, r, *m), (i + 1).into());
    }
}
fn base(t: Target) -> State {
    let mut s = State::new();
    seed(t, &mut s, 0.into(), &[1.into()]);
    seed(t, &mut s, role("SENTINEL_ROLE"), &[88.into()]);
    for (k, v) in [
        (2.into(), 123.into()),
        (3.into(), packed("Synthetic")),
        (4.into(), packed("SYN")),
        (5.into(), 9.into()),
        (6.into(), 1000.into()),
        (pair(t), 77.into()),
        (pair(t) + 1, 78.into()),
        (pair(t) + 2, 79.into()),
        (pair(t) + 3, 1000.into()),
        (pair(t) + 4, 2000.into()),
        (99.into(), U256::MAX),
    ] {
        s.insert(k, v);
    }
    if t == Target::Wkeydao {
        s.insert(8.into(), role("DOMAIN_SENTINEL"));
    }
    s.insert(mapping(44.into(), 0.into()), 123.into());
    s.insert(allowance(44.into(), 55.into()), 9.into());
    s.insert(mapping(44.into(), 7.into()), 17.into());
    s
}
fn pair(t: Target) -> U256 {
    t.role_root() + 1
}
fn allowance(owner: U256, spender: U256) -> U256 {
    mapping(spender, mapping(owner, 1.into()))
}
fn cap() -> U256 {
    U256::from(115_000_000u64) * U256::from(1_000_000_000u64)
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Effect {
    Store(U256, U256, U256),
    Log(Vec<U256>, Vec<u8>),
}
struct Expected {
    state: State,
    effects: Vec<Effect>,
    exit: Exit,
}
impl Expected {
    fn new(pre: &State) -> Self {
        Self {
            state: pre.clone(),
            effects: vec![],
            exit: Exit::Return(vec![]),
        }
    }
    fn store(&mut self, k: U256, v: U256) {
        self.effects.push(Effect::Store(k, get(&self.state, k), v));
        self.state.insert(k, v);
    }
    fn log(&mut self, topics: Vec<U256>, data: Vec<u8>) {
        self.effects.push(Effect::Log(topics, data));
    }
    fn check(&self, name: &str, e: &Execution, pre: &State) -> Result<()> {
        ensure!(e.exit == self.exit, "{name}: actual {:?}, expected {:?}", e.exit, self.exit);
        self.prefix(name, e, pre)?;
        if matches!(e.exit, Exit::Return(_)) {
            ensure!(normalized(&e.committed) == normalized(&self.state), "{name}: all-cell state differs");
            ensure!(e.committed_logs == e.logs, "{name}: complete committed logs");
        } else {
            rollback(name, e, pre)?;
        }
        Ok(())
    }
    fn prefix(&self, name: &str, e: &Execution, pre: &State) -> Result<()> {
        let mut effects: Vec<_> = e
            .writes
            .iter()
            .map(|x| (x.step, Effect::Store(x.key, x.old, x.new)))
            .chain(e.logs.iter().map(|x| (x.step, Effect::Log(x.topics.clone(), x.data.clone()))))
            .collect();
        effects.sort_by_key(|(i, _)| *i);
        ensure!(
            effects.into_iter().map(|(_, x)| x).collect::<Vec<_>>() == self.effects,
            "{name}: exact ordered stores/logs differ; expected {:?}; writes {:?}; logs {:?}",
            self.effects,
            e.writes,
            e.logs
        );
        witnesses(name, e, pre)
    }
}
fn rollback(name: &str, e: &Execution, pre: &State) -> Result<()> {
    ensure!(e.committed == *pre && e.committed_logs.is_empty(), "{name}: rollback");
    Ok(())
}
fn witnesses(name: &str, e: &Execution, pre: &State) -> Result<()> {
    // Reconcile every physical effect with the complete ordered opcode trace.
    for (opcode, count) in [(0x20, e.keccaks.len()), (0x54, e.reads.len()), (0x55, e.writes.len())] {
        ensure!(e.trace.iter().filter(|s| s.opcode == opcode).count() == count, "{name}: omitted opcode witness");
    }
    ensure!(
        e.trace.iter().filter(|s| (0xa0..=0xa4).contains(&s.opcode)).count() == e.logs.len(),
        "{name}: omitted log"
    );
    let mut state = pre.clone();
    let mut stores = e.writes.iter().peekable();
    for read in &e.reads {
        while stores.peek().is_some_and(|s| s.step < read.step) {
            let s = stores.next().unwrap();
            ensure!(get(&state, s.key) == s.old, "{name}: store continuity");
            state.insert(s.key, s.new);
        }
        ensure!(get(&state, read.key) == read.value, "{name}: read continuity");
        let step = e.trace.get(read.step - 1).context("read step")?;
        ensure!(step.pc == read.pc && step.opcode == 0x54, "{name}: read PC");
    }
    for s in stores {
        ensure!(get(&state, s.key) == s.old, "{name}: trailing store continuity");
        state.insert(s.key, s.new);
    }
    for s in &e.writes {
        let step = e.trace.get(s.step - 1).context("write step")?;
        ensure!(step.pc == s.pc && step.opcode == 0x55, "{name}: store PC");
    }
    for h in &e.keccaks {
        ensure!(vm::hash(&h.input) == h.output, "{name}: hash bytes");
        let step = e.trace.get(h.step - 1).context("hash step")?;
        ensure!(step.pc == h.pc && step.opcode == 0x20, "{name}: hash PC");
    }
    for l in &e.logs {
        let step = e.trace.get(l.step - 1).context("log step")?;
        ensure!(step.pc == l.pc && step.opcode == 0xa0 + l.topics.len() as u8, "{name}: log PC");
    }
    Ok(())
}
#[derive(Clone, Copy)]
enum Route {
    Grant,
    Revoke,
    Renounce,
}
impl Route {
    fn signature(self) -> &'static str {
        match self {
            Self::Grant => "grantRole(bytes32,address)",
            Self::Revoke => "revokeRole(bytes32,address)",
            Self::Renounce => "renounceRole(bytes32,address)",
        }
    }
}
fn expected_role(t: Target, pre: &State, r: U256, m: U256, route: Route, caller: U256) -> Expected {
    let mut x = Expected::new(pre);
    let authorized = match route {
        Route::Renounce => caller == m,
        _ => !get(pre, position(t, get(pre, admin(t, r)), caller)).is_zero(),
    };
    if !authorized {
        x.exit = Exit::Revert(error(match route {
            Route::Grant => "AccessControl: sender must be an admin to grant",
            Route::Revoke => "AccessControl: sender must be an admin to revoke",
            Route::Renounce => "AccessControl: can only renounce roles for self",
        }));
        return x;
    }
    let p = get(pre, position(t, r, m));
    let len = get(pre, head(t, r));
    if matches!(route, Route::Grant) && p.is_zero() {
        let next = len.overflowing_add(1.into()).0;
        x.store(head(t, r), next);
        x.store(element(t, r, len), m);
        x.store(position(t, r, m), next);
        x.log(vec![role("RoleGranted(bytes32,address,address)"), r, m, caller], vec![]);
    } else if !matches!(route, Route::Grant) && !p.is_zero() {
        // Solidity 0.7 subtraction wraps; subsequent array accesses enforce
        // bounds with INVALID, rather than a modern arithmetic panic.
        let last = len.overflowing_sub(1.into()).0;
        let at = p - 1;
        if last >= len || at >= len {
            x.exit = Exit::Invalid;
            return x;
        }
        let tail = get(pre, element(t, r, last));
        // This older library always self-swaps, including sole/tail removal.
        x.store(element(t, r, at), tail);
        x.store(position(t, r, tail), p);
        x.store(element(t, r, last), 0.into());
        x.store(head(t, r), last);
        x.store(position(t, r, m), 0.into());
        x.log(vec![role("RoleRevoked(bytes32,address,address)"), r, m, caller], vec![]);
    }
    x
}
pub struct Proof<'a> {
    pub runtime: &'a [u8],
    pub target: Target,
    pub calls: usize,
    pub cases: Vec<Value>,
}
impl<'a> Proof<'a> {
    pub fn new(runtime: &'a [u8], target: Target) -> Self {
        Self {
            runtime,
            target,
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
        data: &[u8],
        caller: U256,
        account: U256,
        pre: &State,
        scope: &str,
        creation: bool,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<Execution> {
        let e = vm::execute_with_self_code_size(code, data, caller, account, pre, Some(if creation { 0 } else { self.target.runtime_len() }));
        self.calls += 1;
        let v = json!({"name":name,"target":self.target.label(),"signature":sig,"scope":scope,"code_kind":if creation {"bytecode"}else{"deployedBytecode"},"code_sha256":super::sha(code),"constructor_code":if creation {Some(hex::encode(code))}else{None},"caller":w(caller),"address":w(account),"self_code_size":if creation {0}else{self.target.runtime_len()},"calldata":hex::encode(data),"prestate":state_json(pre),"execution":execution_json(&e)});
        save(self.calls, &v)?;
        self.cases.push(v);
        if scope != "unsupported_path_control" {
            ensure!(!matches!(e.exit, Exit::HarnessFailure(_)), "{name}: unexpected {:?}", e.exit);
        }
        if scope != "source_invalid_control" {
            ensure!(e.exit != Exit::Invalid, "{name}: unexpected INVALID");
        }
        Ok(e)
    }
    #[allow(clippy::too_many_arguments)]
    fn run(
        &mut self,
        name: &str,
        sig: &str,
        args: &[U256],
        caller: U256,
        pre: &State,
        scope: &str,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<Execution> {
        self.record(
            name,
            sig,
            self.runtime,
            &call(sig, args),
            caller,
            self.target.account(),
            pre,
            scope,
            false,
            save,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn operation(
        &mut self,
        name: &str,
        r: U256,
        m: U256,
        route: Route,
        caller: U256,
        pre: &State,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<State> {
        let want = expected_role(self.target, pre, r, m, route, caller);
        let scope = if want.exit == Exit::Invalid {
            "source_invalid_control"
        } else {
            "synthetic_local_operation"
        };
        let e = self.run(name, route.signature(), &[r, m], caller, pre, scope, save)?;
        want.check(name, &e, pre)?;
        if want.exit == Exit::Invalid {
            ensure!(
                e.trace.last().is_some_and(|s| s.opcode == 0xfe),
                "{name}: actual source INVALID instruction required"
            );
        } else if matches!(e.exit, Exit::Return(_)) {
            let mut reads = vec![position(self.target, r, m)];
            if !matches!(route, Route::Renounce) {
                reads.extend([admin(self.target, r), position(self.target, get(pre, admin(self.target, r)), caller)]);
            }
            for key in reads {
                ensure!(e.reads.iter().any(|x| x.key == key), "{name}: authorization/position read omitted");
            }
            for input in [
                words(&[r, self.target.role_root()]),
                words(&[m, head(self.target, r).overflowing_add(1.into()).0]),
            ] {
                ensure!(e.keccaks.iter().any(|h| h.input == input), "{name}: role/index preimage omitted");
            }
            if !e.writes.is_empty() {
                ensure!(
                    e.keccaks.iter().any(|h| h.input == vm::word(head(self.target, r))),
                    "{name}: array preimage omitted"
                );
            }
        }
        Ok(e.committed)
    }
    fn getter(&mut self, name: &str, sig: &str, args: &[U256], value: U256, pre: &State, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let e = self.run(name, sig, args, 9.into(), pre, "synthetic_local_operation", save)?;
        let mut x = Expected::new(pre);
        x.exit = Exit::Return(vm::word(value).to_vec());
        x.check(name, &e, pre)?;
        ensure!(e.committed == *pre, "{name}: getter cannot normalize explicit zero cells");
        Ok(())
    }
    fn readbacks(&mut self, name: &str, r: U256, member: U256, pre: &State, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let len = get(pre, head(self.target, r));
        ensure!(len < 16.into(), "bounded coherent readbacks");
        self.getter(name, "getRoleMemberCount(bytes32)", &[r], len, pre, save)?;
        self.getter(name, "getRoleAdmin(bytes32)", &[r], get(pre, admin(self.target, r)), pre, save)?;
        let mut members = BTreeSet::from([member, 0.into(), 99.into()]);
        for i in 0..len.as_usize() {
            let m = get(pre, element(self.target, r, i.into()));
            members.insert(m);
            self.getter(
                name,
                "getRoleMember(bytes32,uint256)",
                &[r, i.into()],
                m & ((U256::one() << 160) - 1),
                pre,
                save,
            )?;
        }
        for m in members {
            self.getter(
                name,
                "hasRole(bytes32,address)",
                &[r, m],
                u8::from(!get(pre, position(self.target, r, m)).is_zero()).into(),
                pre,
                save,
            )?;
        }
        for i in [len, U256::MAX] {
            let e = self.run(
                name,
                "getRoleMember(bytes32,uint256)",
                &[r, i],
                9.into(),
                pre,
                "synthetic_local_operation",
                save,
            )?;
            let mut x = Expected::new(pre);
            x.exit = Exit::Revert(error("EnumerableSet: index out of bounds"));
            x.check(name, &e, pre)?;
        }
        for (sig, args, value) in [
            ("balanceOf(address)", vec![44.into()], get(pre, mapping(44.into(), 0.into()))),
            ("totalSupply()", vec![], get(pre, 2.into())),
            (
                "allowance(address,address)",
                vec![44.into(), 55.into()],
                get(pre, mapping(55.into(), mapping(44.into(), 1.into()))),
            ),
        ] {
            self.getter(name, sig, &args, value, pre, save)?;
        }
        Ok(())
    }
    pub fn matrix(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let r = role("TEST_ROLE");
        for (name, before, member, route) in [
            ("role_add_empty", vec![], 3, Route::Grant),
            ("role_add_nonempty", vec![3, 4], 5, Route::Grant),
            ("role_add_zero_empty", vec![], 0, Route::Grant),
            ("role_add_zero_nonempty", vec![3, 4], 0, Route::Grant),
            ("role_duplicate", vec![3, 4], 3, Route::Grant),
            ("role_duplicate_zero", vec![0, 4], 0, Route::Grant),
            ("role_remove_first", vec![3, 4, 5], 3, Route::Revoke),
            ("role_remove_middle", vec![3, 4, 5], 4, Route::Revoke),
            ("role_remove_tail", vec![3, 4, 5], 5, Route::Revoke),
            ("role_remove_sole", vec![3], 3, Route::Revoke),
            ("role_remove_zero_sole", vec![0], 0, Route::Revoke),
            ("role_remove_zero_first", vec![0, 4, 5], 0, Route::Revoke),
            ("role_remove_zero_tail", vec![3, 4, 0], 0, Route::Revoke),
            ("role_move_zero_tail", vec![3, 4, 0], 3, Route::Revoke),
            ("role_absent_empty", vec![], 3, Route::Revoke),
            ("role_absent_nonempty", vec![4, 5], 3, Route::Revoke),
            ("renounce_self", vec![3, 4, 5], 3, Route::Renounce),
            ("renounce_tail", vec![3, 4], 4, Route::Renounce),
            ("renounce_zero", vec![0], 0, Route::Renounce),
            ("renounce_absent", vec![4], 3, Route::Renounce),
        ] {
            let mut pre = base(self.target);
            let before: Vec<U256> = before.into_iter().map(U256::from).collect();
            seed(self.target, &mut pre, r, &before);
            let m = U256::from(member);
            let caller = if matches!(route, Route::Renounce) { m } else { 1.into() };
            let post = self.operation(name, r, m, route, caller, &pre, save)?;
            self.readbacks(name, r, m, &post, save)?;
        }
        for (i, r) in [U256::zero(), role("MINT"), role("INTERN_SYSTEM"), U256::MAX, (U256::one() << 255) + 7]
            .into_iter()
            .enumerate()
        {
            let pre = base(self.target);
            let name = format!("role_identity_{i}");
            let post = self.operation(&name, r, 3.into(), Route::Grant, 1.into(), &pre, save)?;
            self.readbacks(&name, r, 3.into(), &post, save)?;
        }
        let mut pre = base(self.target);
        let custom = role("CUSTOM_ADMIN");
        seed(self.target, &mut pre, custom, &[2.into()]);
        pre.insert(admin(self.target, r), custom);
        let post = self.operation("custom_admin_authorized", r, 3.into(), Route::Grant, 2.into(), &pre, save)?;
        self.readbacks("custom_admin_authorized", r, 3.into(), &post, save)?;
        for (name, route, caller) in [
            ("wrong_grant_admin", Route::Grant, 1),
            ("wrong_revoke_admin", Route::Revoke, 1),
            ("wrong_renounce_account", Route::Renounce, 2),
        ] {
            self.operation(name, r, 3.into(), route, caller.into(), &post, save)?;
        }
        let mut max_admin = base(self.target);
        seed(self.target, &mut max_admin, U256::MAX, &[2.into()]);
        max_admin.insert(admin(self.target, r), U256::MAX);
        let max_post = self.operation("max_admin_authorized", r, 3.into(), Route::Grant, 2.into(), &max_admin, save)?;
        self.readbacks("max_admin_authorized", r, 3.into(), &max_post, save)?;
        self.operation("max_admin_revoke", r, 3.into(), Route::Revoke, 2.into(), &max_post, save)?;
        let no_admin = self.operation("last_admin_renounce", 0.into(), 1.into(), Route::Renounce, 1.into(), &base(self.target), save)?;
        self.readbacks("last_admin_renounce", 0.into(), 1.into(), &no_admin, save)?;
        self.operation("grant_after_last_admin_renounce", r, 3.into(), Route::Grant, 1.into(), &no_admin, save)?;
        self.operation("repeat_last_admin_renounce", 0.into(), 1.into(), Route::Renounce, 1.into(), &no_admin, save)?;
        let mut sequence = base(self.target);
        for (i, (route, m)) in [
            (Route::Grant, 3),
            (Route::Grant, 0),
            (Route::Grant, 4),
            (Route::Grant, 3),
            (Route::Revoke, 3),
            (Route::Renounce, 0),
            (Route::Revoke, 4),
            (Route::Grant, 3),
            (Route::Revoke, 3),
        ]
        .into_iter()
        .enumerate()
        {
            let name = format!("sequence_{i}");
            let m = U256::from(m);
            let caller = if matches!(route, Route::Renounce) { m } else { 1.into() };
            sequence = self.operation(&name, r, m, route, caller, &sequence, save)?;
            self.readbacks(&name, r, m, &sequence, save)?;
        }
        ensure!(normalized(&sequence) == normalized(&base(self.target)), "sequential restoration");
        self.malformed(save)?;
        self.raw_getters(save)?;
        self.token_controls(save)?;
        self.excluded(save)?;
        Ok(())
    }
    fn malformed(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let r = role("MALFORMED_ROLE");
        let m = U256::from(3);
        for (name, len, pos) in [
            ("empty_nonzero_position", U256::zero(), U256::one()),
            ("out_of_bounds_position", 1.into(), 2.into()),
            ("max_position", 1.into(), U256::MAX),
        ] {
            let mut pre = base(self.target);
            pre.insert(head(self.target, r), len);
            pre.insert(position(self.target, r, m), pos);
            pre.insert(element(self.target, r, 0.into()), m);
            self.operation(name, r, m, Route::Revoke, 1.into(), &pre, save)?;
        }
        // Malformed successes are source controls, not coherent admission.
        let mut pre = base(self.target);
        pre.insert(head(self.target, r), U256::MAX);
        self.operation("malformed_max_length_push", r, m, Route::Grant, 1.into(), &pre, save)?;
        let mut pre = base(self.target);
        pre.insert(head(self.target, r), 1.into());
        pre.insert(element(self.target, r, 0.into()), 4.into());
        pre.insert(position(self.target, r, m), 1.into());
        pre.insert(position(self.target, r, 4.into()), 1.into());
        self.operation("malformed_index_points_to_other_member", r, m, Route::Revoke, 1.into(), &pre, save)?;
        let mut pre = base(self.target);
        seed(self.target, &mut pre, r, &[3.into(), 4.into()]);
        pre.insert(element(self.target, r, 1.into()), (U256::one() << 200) + 4);
        self.operation("malformed_dirty_tail_word", r, m, Route::Revoke, 1.into(), &pre, save)?;
        let mut pre = base(self.target);
        pre.insert(position(self.target, r, m), U256::MAX);
        self.operation("malformed_duplicate_ignores_length", r, m, Route::Grant, 1.into(), &pre, save)?;
        let ordinary = base(self.target);
        for (name, data) in [
            ("unknown_selector", vec![0xff; 4]),
            ("empty_calldata", vec![]),
            ("truncated_role_abi", call("grantRole(bytes32,address)", &[r])),
        ] {
            let e = self.record(
                name,
                "malformed ABI",
                self.runtime,
                &data,
                1.into(),
                self.target.account(),
                &ordinary,
                "synthetic_local_operation",
                false,
                save,
            )?;
            let mut x = Expected::new(&ordinary);
            x.exit = Exit::Revert(vec![]);
            x.check(name, &e, &ordinary)?;
        }
        // Legacy ABI semantics must be measured, not inferred from modern solc.
        let dirty = (U256::one() << 200) + 3;
        let data = call("grantRole(bytes32,address)", &[r, dirty]);
        let e = self.record(
            "dirty_address_abi",
            "grantRole(bytes32,address)",
            self.runtime,
            &data,
            1.into(),
            self.target.account(),
            &ordinary,
            "synthetic_local_operation",
            false,
            save,
        )?;
        expected_role(self.target, &ordinary, r, 3.into(), Route::Grant, 1.into()).check("dirty_address_abi", &e, &ordinary)?;
        let extra = [call("grantRole(bytes32,address)", &[r, 3.into()]), vec![255; 32]].concat();
        let e = self.record(
            "trailing_role_abi",
            "grantRole(bytes32,address)",
            self.runtime,
            &extra,
            1.into(),
            self.target.account(),
            &ordinary,
            "synthetic_local_operation",
            false,
            save,
        )?;
        expected_role(self.target, &ordinary, r, 3.into(), Route::Grant, 1.into()).check("trailing_role_abi", &e, &ordinary)?;
        let e = self.run(
            "no_public_admin_setter",
            "setRoleAdmin(bytes32,bytes32)",
            &[r, role("OTHER")],
            1.into(),
            &ordinary,
            "synthetic_local_operation",
            save,
        )?;
        let mut x = Expected::new(&ordinary);
        x.exit = Exit::Revert(vec![]);
        x.check("no_public_admin_setter", &e, &ordinary)?;
        Ok(())
    }
}

fn unsupported(name: &str, e: &Execution, pre: &State, opcode: u8) -> Result<()> {
    let last = e.trace.last().context("unsupported trace")?;
    ensure!(last.opcode == opcode, "{name}: exact unsupported boundary");
    ensure!(
        matches!(&e.exit, Exit::HarnessFailure(_)),
        "{name}: unsupported context is not a source revert/INVALID"
    );
    if opcode != 0x3b {
        ensure!(
            e.exit == Exit::HarnessFailure(format!("unsupported opcode 0x{opcode:02x} at pc {}", last.pc)),
            "{name}: exact harness error"
        );
    }
    ensure!(
        !e.trace.iter().any(|s| matches!(s.opcode, 0xf1 | 0xf4 | 0xfa)),
        "{name}: no external calls executed"
    );
    rollback(name, e, pre)
}

// Expected state/effects come from the selected Solidity bodies; VM outputs are
// compared only after the raw attempt is saved. Arithmetic order is intentional.
impl Expected {
    fn fail(&mut self, message: &str) {
        self.exit = Exit::Revert(error(message));
    }
    fn add(&mut self, a: U256, b: U256) -> Option<U256> {
        let (c, overflow) = a.overflowing_add(b);
        if overflow {
            self.fail("SafeMath: addition overflow");
            None
        } else {
            Some(c)
        }
    }
    fn sub(&mut self, a: U256, b: U256, message: &str) -> Option<U256> {
        if b > a {
            self.fail(message);
            None
        } else {
            Some(a - b)
        }
    }
    fn transfer_log(&mut self, from: U256, to: U256, amount: U256) {
        self.log(vec![role("Transfer(address,address,uint256)"), from, to], words(&[amount]));
    }
    fn setup(&mut self, t: Target, r: U256, m: U256, caller: U256) {
        if get(&self.state, position(t, r, m)).is_zero() {
            let len = get(&self.state, head(t, r));
            let next = len.overflowing_add(1.into()).0;
            self.store(head(t, r), next);
            self.store(element(t, r, len), m);
            self.store(position(t, r, m), next);
            self.log(vec![role("RoleGranted(bytes32,address,address)"), r, m, caller], vec![]);
        }
    }
    fn approve(&mut self, owner: U256, spender: U256, amount: U256) -> bool {
        if owner.is_zero() {
            self.fail("ERC20: approve from the zero address");
            return false;
        }
        if spender.is_zero() {
            self.fail("ERC20: approve to the zero address");
            return false;
        }
        self.store(allowance(owner, spender), amount);
        self.log(vec![role("Approval(address,address,uint256)"), owner, spender], words(&[amount]));
        true
    }
    fn burn(&mut self, account: U256, amount: U256) {
        if account.is_zero() {
            self.fail("ERC20: burn from the zero address");
            return;
        }
        let key = mapping(account, 0.into());
        let Some(b) = self.sub(get(&self.state, key), amount, "ERC20: burn amount exceeds balance") else {
            return;
        };
        self.store(key, b);
        let Some(supply) = self.sub(get(&self.state, 2.into()), amount, "SafeMath: subtraction overflow") else {
            return;
        };
        self.store(2.into(), supply);
        let Some(cap) = self.sub(get(&self.state, 6.into()), amount, "SafeMath: subtraction overflow") else {
            return;
        };
        self.store(6.into(), cap);
        self.transfer_log(account, 0.into(), amount);
    }
    fn transfer(&mut self, t: Target, from: U256, to: U256, mut amount: U256) {
        if from.is_zero() {
            self.fail("ERC20: transfer from the zero address");
            return;
        }
        if to.is_zero() {
            self.fail("ERC20: transfer to the zero address");
            return;
        }
        let from_key = mapping(from, 0.into());
        let Some(b) = self.sub(get(&self.state, from_key), amount, "ERC20: transfer amount exceeds balance") else {
            return;
        };
        self.store(from_key, b);
        let p = get(&self.state, pair(t));
        let is_trade = (from == p && get(&self.state, position(t, role("INTERN_SYSTEM"), to)).is_zero())
            || (to == p && get(&self.state, position(t, role("INTERN_SYSTEM"), from)).is_zero());
        if is_trade {
            let buy = from == p;
            let ratio = get(&self.state, pair(t) + if buy { 4 } else { 3 });
            let (product, overflow) = amount.overflowing_mul(ratio);
            if overflow {
                self.fail("SafeMath: multiplication overflow");
                return;
            }
            let fee = product / U256::from(100000);
            if !fee.is_zero() {
                amount = amount.overflowing_sub(fee).0;
                let receiver = get(&self.state, pair(t) + if buy { 2 } else { 1 }) & ((U256::one() << 160) - 1);
                let k = mapping(receiver, 0.into());
                self.store(k, get(&self.state, k).overflowing_add(fee).0);
                self.transfer_log(from, receiver, fee);
                if !buy {
                    self.log(vec![role("FeeTaken(address,address,uint256,uint256)"), from, receiver], words(&[amount, fee]));
                    self.exit = Exit::HarnessFailure("EXTCODESIZE unknown external account".into());
                    return;
                }
            }
        }
        let k = mapping(to, 0.into());
        let Some(b) = self.add(get(&self.state, k), amount) else { return };
        self.store(k, b);
        self.transfer_log(from, to, amount);
    }
}
fn expected_mint(t: Target, pre: &State, caller: U256, account: U256, amount: U256) -> Expected {
    let mut x = Expected::new(pre);
    if get(pre, position(t, role("MINT"), caller)).is_zero() {
        x.fail("VaultOwned: caller is not the Vault");
        return x;
    }
    let Some(total) = x.add(get(pre, 2.into()), amount) else { return x };
    if total > get(pre, 6.into()) {
        x.fail("ERC20: mint amount exceeds max supply");
        return x;
    }
    if account.is_zero() {
        x.fail("ERC20: mint to the zero address");
        return x;
    }
    x.store(2.into(), total);
    let k = mapping(account, 0.into());
    let Some(balance) = x.add(get(&x.state, k), amount) else { return x };
    x.store(k, balance);
    x.transfer_log(t.account(), account, amount);
    x
}
impl Proof<'_> {
    pub fn constructor(&mut self, capture: &Value, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let t = self.target;
        ensure!(
            address(capture["address"].as_str().context("capture address")?) == t.account(),
            "target constructor"
        );
        let (fee, buy, name, symbol) = match t {
            Target::Wkeydao => (
                "0x1e92d477473295e9f3b0f630f010b4ef8658da94",
                "0xb63f6fe69dcaa4ec43903067ef2545edcb4b6ca7",
                "WebKey DAO",
                "wkeyDAO",
            ),
            Target::Got => (
                "0xa6ffd3255b442b1f14787b39cc731a709d64fbd5",
                "0xa8324322c57403af580b5a7896bad7f138a7837b",
                "GOLDEN PACT",
                "GOT",
            ),
        };
        let args = bytes(&capture["creationBytecode"]["transformationValues"]["constructorArguments"])?;
        ensure!(
            args == words(&[address(fee), address(buy), 60000.into()]),
            "independent full three-word constructor ABI"
        );
        let creation = bytes(&capture["creationBytecode"]["recompiledBytecode"])?;
        ensure!(creation.len() == t.creation_len(), "selected creation length");
        ensure!(
            [creation.clone(), args].concat() == bytes(&capture["creationBytecode"]["onchainBytecode"])?,
            "complete saved creation binding"
        );
        let caller = address(capture["deployment"]["deployer"].as_str().context("deployer")?);
        let mut variants = vec![(format!("{}_constructor", t.label()), address(fee), address(buy), 60000.into())];
        if t == Target::Got {
            variants.extend([
                ("got_constructor_zero_fee".into(), 0.into(), address(buy), 60000.into()),
                ("got_constructor_zero_buy".into(), address(fee), 0.into(), 60000.into()),
                ("got_constructor_bad_ratio".into(), address(fee), address(buy), 100001.into()),
                ("got_constructor_zero_ratio".into(), address(fee), address(buy), 0.into()),
                ("got_constructor_max_ratio".into(), address(fee), address(buy), 100000.into()),
                ("got_constructor_fee_deployer_alias".into(), caller, address(buy), 60000.into()),
            ]);
        }
        for (label, fee, buy, ratio) in variants {
            let code = [creation.clone(), words(&[fee, buy, ratio])].concat();
            let pre = State::new();
            let scope = if t == Target::Wkeydao {
                "unsupported_path_control"
            } else {
                "synthetic_constructor_control"
            };
            let e = self.record(
                &label,
                "constructor(address,address,uint256)",
                &code,
                &[],
                caller,
                t.account(),
                &pre,
                scope,
                true,
                save,
            )?;
            let mut x = Expected::new(&pre);
            // Declaration initializer is emitted before base constructors in
            // each full creation program. Names/cap then precede permit logic.
            x.store(pair(t) + 3, if t == Target::Wkeydao { 60000.into() } else { 3000.into() });
            x.store(3.into(), packed(name));
            x.store(4.into(), packed(symbol));
            x.store(5.into(), 9.into());
            x.store(6.into(), cap());
            if t == Target::Wkeydao {
                x.prefix(&label, &e, &pre)?;
                unsupported(&label, &e, &pre, 0x46)?;
                continue;
            }
            x.setup(t, 0.into(), caller, caller); // VaultOwned base constructor.
            if fee.is_zero() {
                x.fail("Invalid fee receiver")
            } else if buy.is_zero() {
                x.fail("Invalid buy fee receiver")
            } else if ratio > 100000.into() {
                x.fail("Invalid buy fee ratio")
            } else {
                x.store(pair(t) + 1, fee);
                x.store(pair(t) + 2, buy);
                x.store(pair(t) + 4, ratio);
                x.setup(t, 0.into(), caller, caller); // Duplicate in GOT body: no effects.
                x.setup(t, role("INTERN_SYSTEM"), caller, caller);
                x.setup(t, role("INTERN_SYSTEM"), fee, caller);
                x.exit = Exit::Return(self.runtime.to_vec());
            }
            x.check(&label, &e, &pre)?;
        }
        Ok(())
    }
    fn raw_getters(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let t = self.target;
        for (i, value) in [U256::zero(), 1.into(), U256::one() << 255, U256::MAX].into_iter().enumerate() {
            let mut pre = base(t);
            let mut specs = vec![
                ("balanceOf(address)", vec![44.into()], mapping(44.into(), 0.into()), false),
                ("totalSupply()", vec![], 2.into(), false),
                ("MaxSupply()", vec![], 6.into(), false),
                ("allowance(address,address)", vec![44.into(), 55.into()], allowance(44.into(), 55.into()), false),
                ("nonces(address)", vec![44.into()], mapping(44.into(), 7.into()), false),
                ("mainPair()", vec![], pair(t), true),
                ("feeReceiver()", vec![], pair(t) + 1, true),
                ("buyFeeReceiver()", vec![], pair(t) + 2, true),
                ("feeRatio()", vec![], pair(t) + 3, false),
                ("buyFeeRatio()", vec![], pair(t) + 4, false),
            ];
            if t == Target::Wkeydao {
                specs.push(("DOMAIN_SEPARATOR()", vec![], 8.into(), false));
            }
            for (_, _, k, _) in &specs {
                pre.insert(*k, value);
            }
            for (sig, args, key, mask) in specs {
                let name = format!("raw_getter_{i}");
                let want = if mask { value & ((U256::one() << 160) - 1) } else { value };
                self.getter(&name, sig, &args, want, &pre, save)?;
                let last = self.cases.last().context("getter saved")?;
                ensure!(
                    last["execution"]["reads"].as_array().context("reads")?.len() == 1 && last["execution"]["reads"][0]["key"] == w(key),
                    "{sig}: exact single raw read"
                );
            }
        }
        let pre = base(t);
        for (sig, s) in [("name()", "Synthetic"), ("symbol()", "SYN")] {
            let e = self.run("raw_string_getter", sig, &[], 9.into(), &pre, "synthetic_local_operation", save)?;
            let mut x = Expected::new(&pre);
            x.exit = Exit::Return([words(&[32.into()]), string_tail(s)].concat());
            x.check(sig, &e, &pre)?;
        }
        for (sig, value) in [
            ("decimals()", 9.into()),
            ("PRECISION()", 100000.into()),
            ("MINT()", role("MINT")),
            ("INTERN_SYSTEM()", role("INTERN_SYSTEM")),
            ("DEFAULT_ADMIN_ROLE()", 0.into()),
        ] {
            self.getter("raw_constant_getter", sig, &[], value, &pre, save)?;
        }
        if t == Target::Got {
            let e = self.run(
                "got_has_no_domain_getter",
                "DOMAIN_SEPARATOR()",
                &[],
                9.into(),
                &pre,
                "synthetic_local_operation",
                save,
            )?;
            let mut x = Expected::new(&pre);
            x.exit = Exit::Revert(vec![]);
            x.check("GOT no domain storage", &e, &pre)?;
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn check_token(
        &mut self,
        name: &str,
        sig: &str,
        args: &[U256],
        caller: U256,
        pre: &State,
        x: Expected,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<()> {
        let scope = if matches!(x.exit, Exit::HarnessFailure(_)) {
            "unsupported_path_control"
        } else {
            "synthetic_local_operation"
        };
        let e = self.run(name, sig, args, caller, pre, scope, save)?;
        x.check(name, &e, pre)?;
        if matches!(x.exit, Exit::HarnessFailure(_)) {
            unsupported(name, &e, pre, 0x3b)?;
        }
        Ok(())
    }
    fn token_controls(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let t = self.target;
        // Exact cap order, authorized/unauthorized, zero, and late balance overflow.
        for (name, caller, to, amount, supply, max, balance) in [
            ("mint_ordinary", 2, 44, 7.into(), 123.into(), 1000.into(), 123.into()),
            ("mint_cap_exact", 2, 44, 877.into(), 123.into(), 1000.into(), 123.into()),
            ("mint_cap_excess", 2, 44, 878.into(), 123.into(), 1000.into(), 123.into()),
            ("mint_unauthorized", 3, 44, 1.into(), 123.into(), 1000.into(), 123.into()),
            ("mint_zero_amount", 2, 44, 0.into(), 123.into(), 1000.into(), 123.into()),
            ("mint_zero_account", 2, 0, 0.into(), 123.into(), 1000.into(), 0.into()),
            ("mint_cap_before_zero_account", 2, 0, 1.into(), 123.into(), 123.into(), 0.into()),
            ("mint_supply_overflow", 2, 44, 1.into(), U256::MAX, U256::MAX, 0.into()),
            ("mint_balance_overflow", 2, 44, 1.into(), 0.into(), 1000.into(), U256::MAX),
        ] {
            let mut pre = base(t);
            seed(t, &mut pre, role("MINT"), &[2.into()]);
            pre.insert(2.into(), supply);
            pre.insert(6.into(), max);
            pre.insert(mapping(U256::from(to), 0.into()), balance);
            let x = expected_mint(t, &pre, caller.into(), to.into(), amount);
            self.check_token(name, "mint(address,uint256)", &[to.into(), amount], caller.into(), &pre, x, save)?;
        }
        for (name, caller, amount, balance, supply, max) in [
            ("burn_ordinary", 44, 7, 123, 123, 1000),
            ("burn_zero_amount", 44, 0, 123, 123, 1000),
            ("burn_all", 44, 123, 123, 123, 1000),
            ("burn_zero_account", 0, 0, 0, 0, 0),
            ("burn_balance_failure", 44, 124, 123, 123, 1000),
            ("burn_supply_failure", 44, 7, 123, 6, 1000),
            ("burn_cap_failure", 44, 7, 123, 123, 6),
        ] {
            let mut pre = base(t);
            pre.insert(mapping(caller.into(), 0.into()), balance.into());
            pre.insert(2.into(), supply.into());
            pre.insert(6.into(), max.into());
            let mut x = Expected::new(&pre);
            x.burn(caller.into(), amount.into());
            self.check_token(name, "burn(uint256)", &[amount.into()], caller.into(), &pre, x, save)?;
        }
        for sig in ["burnFrom(address,uint256)", "_burnFrom(address,uint256)"] {
            for (name, owner, caller, amount, allowed, balance, supply, max) in [
                ("burn_from_ordinary", 44, 55, 7, 9, 123, 123, 1000),
                ("burn_from_zero", 44, 55, 0, 9, 123, 123, 1000),
                ("burn_from_allowance_failure", 44, 55, 10, 9, 123, 123, 1000),
                ("burn_from_balance_failure", 44, 55, 7, 9, 6, 123, 1000),
                ("burn_from_supply_failure", 44, 55, 7, 9, 123, 6, 1000),
                ("burn_from_cap_failure", 44, 55, 7, 9, 123, 123, 6),
                ("burn_from_zero_owner", 0, 55, 0, 0, 0, 123, 1000),
                ("burn_from_zero_spender", 44, 0, 0, 9, 123, 123, 1000),
            ] {
                let mut pre = base(t);
                pre.insert(allowance(owner.into(), caller.into()), allowed.into());
                pre.insert(mapping(owner.into(), 0.into()), balance.into());
                pre.insert(2.into(), supply.into());
                pre.insert(6.into(), max.into());
                let mut x = Expected::new(&pre);
                if t == Target::Got && sig.starts_with('_') {
                    x.exit = Exit::Revert(vec![]);
                } else if let Some(left) = x.sub(allowed.into(), amount.into(), "ERC20: burn amount exceeds allowance") {
                    if x.approve(owner.into(), caller.into(), left) {
                        x.burn(owner.into(), amount.into());
                    }
                }
                self.check_token(name, sig, &[owner.into(), amount.into()], caller.into(), &pre, x, save)?;
            }
        }
        self.setters(save)?;
        self.transfers(save)?;
        Ok(())
    }
    fn setters(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let t = self.target;
        for (name, caller, value) in [
            ("pair_change", 1, 80),
            ("pair_equality", 1, 77),
            ("pair_zero", 1, 0),
            ("pair_unauthorized", 2, 80),
        ] {
            let pre = base(t);
            let mut x = Expected::new(&pre);
            if caller != 1 {
                x.fail("Caller is not admin")
            } else {
                x.store(pair(t), value.into());
            }
            self.check_token(name, "setMainPair(address)", &[value.into()], caller.into(), &pre, x, save)?;
        }
        for ty in [0u8, 1, 2, 255] {
            for (suffix, caller, value) in [
                ("zero", 1, U256::zero()),
                ("equal", 1, if ty == 0 { 2000.into() } else { 1000.into() }),
                ("maximum", 1, 100000.into()),
                ("too_large", 1, 100001.into()),
                ("max_word", 1, U256::MAX),
                ("unauthorized", 2, 0.into()),
            ] {
                let name = format!("ratio_{ty}_{suffix}");
                let pre = base(t);
                let mut x = Expected::new(&pre);
                if caller != 1 {
                    x.fail("Caller is not admin");
                } else if value > 100000.into() {
                    x.fail("Exceeds precision");
                } else {
                    x.store(pair(t) + if ty == 0 { 4 } else { 3 }, value);
                    x.log(vec![role("FeeRatioChanged(uint8,uint256)")], words(&[ty.into(), value]));
                }
                self.check_token(&name, "setRatio(uint8,uint256)", &[ty.into(), value], caller.into(), &pre, x, save)?;
            }
        }
        Ok(())
    }
    fn transfers(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let t = self.target;
        // The deliberately overflowing receiver case is a malformed-prestate
        // source arithmetic control, not a coherent token-state claim.
        for (name, from, to, amount, ratio, bypass, receiver, receiver_balance) in [
            ("transfer_ordinary", 44, 45, 7, 2000, false, 79, U256::zero()),
            ("transfer_self", 44, 44, 7, 2000, false, 79, U256::zero()),
            ("transfer_zero", 44, 45, 0, 2000, false, 79, U256::zero()),
            ("transfer_zero_sender", 0, 45, 0, 2000, false, 79, U256::zero()),
            ("transfer_zero_recipient", 44, 0, 0, 2000, false, 79, U256::zero()),
            ("transfer_insufficient", 44, 45, 124, 2000, false, 79, U256::zero()),
            ("buy_fee", 77, 45, 100, 2000, false, 79, U256::zero()),
            ("buy_dust", 77, 45, 1, 2000, false, 79, U256::zero()),
            ("buy_zero_ratio", 77, 45, 100, 0, false, 79, U256::zero()),
            ("buy_fee_recipient_alias", 77, 45, 100, 2000, false, 45, U256::zero()),
            ("buy_fee_sender_alias", 77, 45, 100, 2000, false, 77, U256::from(100)),
            ("buy_fee_unchecked_receiver_wrap", 77, 45, 100, 2000, false, 79, U256::MAX),
            ("buy_intern_bypass", 77, 45, 100, 2000, true, 79, U256::zero()),
            ("sell_intern_bypass", 44, 77, 100, 2000, true, 79, U256::zero()),
            ("sell_dust_zero_fee", 44, 77, 1, 2000, false, 79, U256::zero()),
        ] {
            let mut pre = base(t);
            if from == 77 {
                pre.insert(mapping(77.into(), 0.into()), 100.into());
                pre.insert(2.into(), 223.into());
            }
            pre.insert(pair(t) + 4, ratio.into());
            pre.insert(pair(t) + 2, receiver.into());
            if receiver != 77 {
                pre.insert(mapping(receiver.into(), 0.into()), receiver_balance);
            }
            if bypass {
                seed(t, &mut pre, role("INTERN_SYSTEM"), &[if from == 77 { to.into() } else { from.into() }]);
            }
            let mut x = Expected::new(&pre);
            x.transfer(t, from.into(), to.into(), amount.into());
            if matches!(x.exit, Exit::Return(_)) {
                x.exit = Exit::Return(words(&[1.into()]));
            }
            self.check_token(name, "transfer(address,uint256)", &[to.into(), amount.into()], from.into(), &pre, x, save)?;
        }
        for (name, allowed) in [("transfer_from_success", 9), ("transfer_from_late_allowance_failure", 6)] {
            let mut pre = base(t);
            pre.insert(allowance(44.into(), 55.into()), allowed.into());
            let mut x = Expected::new(&pre);
            x.transfer(t, 44.into(), 45.into(), 7.into());
            if let Some(left) = x.sub(allowed.into(), 7.into(), "ERC20: transfer amount exceeds allowance") {
                x.approve(44.into(), 55.into(), left);
                x.exit = Exit::Return(words(&[1.into()]));
            }
            self.check_token(
                name,
                "transferFrom(address,address,uint256)",
                &[44.into(), 45.into(), 7.into()],
                55.into(),
                &pre,
                x,
                save,
            )?;
        }
        let mut pre = base(t);
        pre.insert(mapping(45.into(), 0.into()), U256::MAX);
        let mut x = Expected::new(&pre);
        x.transfer(t, 44.into(), 45.into(), 1.into());
        self.check_token(
            "transfer_recipient_add_overflow",
            "transfer(address,uint256)",
            &[45.into(), 1.into()],
            44.into(),
            &pre,
            x,
            save,
        )?;
        let mut pre = base(t);
        pre.insert(mapping(77.into(), 0.into()), 2.into());
        pre.insert(pair(t) + 4, U256::MAX);
        let mut x = Expected::new(&pre);
        x.transfer(t, 77.into(), 45.into(), 2.into());
        self.check_token(
            "buy_multiply_overflow",
            "transfer(address,uint256)",
            &[45.into(), 2.into()],
            77.into(),
            &pre,
            x,
            save,
        )?;
        Ok(())
    }
    fn excluded(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let t = self.target;
        let pre = base(t);
        let mut x = Expected::new(&pre);
        x.transfer(t, 44.into(), 77.into(), 100.into());
        self.check_token(
            "excluded_fee_receiver",
            "transfer(address,uint256)",
            &[77.into(), 100.into()],
            44.into(),
            &pre,
            x,
            save,
        )?;
        let e = self.run(
            "excluded_permit",
            "permit(address,address,uint256,uint256,uint8,bytes32,bytes32)",
            &[44.into(), 55.into(), 1.into(), U256::MAX, 27.into(), 1.into(), 1.into()],
            44.into(),
            &pre,
            "unsupported_path_control",
            save,
        )?;
        Expected::new(&pre).prefix("excluded_permit", &e, &pre)?;
        unsupported("excluded_permit", &e, &pre, 0x42)?;
        Ok(())
    }
}
