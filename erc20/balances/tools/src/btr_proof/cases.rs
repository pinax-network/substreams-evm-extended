//! Synthetic local operation proof for the captured BTR implementation.
//! This does not execute proxy dispatch or admit storage into production.
use super::{address, vm, CAPTURES, IMPLEMENTATION, PROXY};
use crate::ptoken_proof::cases::{call, execution_json, get, mapping, role, state_json, w};
use anyhow::{ensure, Context, Result};
use primitive_types::U256;
use serde_json::{json, Value};
use vm::{Execution, Exit, State};

pub fn member_key(r: U256, member: U256) -> U256 {
    mapping(member, mapping(r, 101.into()))
}
pub fn role_admin(r: U256) -> U256 {
    mapping(r, 101.into()).overflowing_add(1.into()).0
}
#[derive(Clone, Copy)]
enum Set {
    Role(U256),
    Whitelist,
}
impl Set {
    fn head(self) -> U256 {
        match self {
            Self::Role(r) => mapping(r, 151.into()),
            Self::Whitelist => 555.into(),
        }
    }
    fn element(self, index: U256) -> U256 {
        vm::hash(&vm::word(self.head())).overflowing_add(index).0
    }
    fn position(self, member: U256) -> U256 {
        mapping(member, self.head().overflowing_add(1.into()).0)
    }
}
fn normalize(s: &State) -> State {
    s.iter().filter(|(_, v)| !v.is_zero()).map(|(k, v)| (*k, *v)).collect()
}
fn replace_set(s: &mut State, set: Set, members: &[U256]) {
    for i in 0..get(s, set.head()).as_usize() {
        let m = get(s, set.element(i.into()));
        s.remove(&set.position(m));
        s.remove(&set.element(i.into()));
        if let Set::Role(r) = set {
            s.remove(&member_key(r, m));
        }
    }
    s.insert(set.head(), members.len().into());
    for (i, m) in members.iter().enumerate() {
        s.insert(set.element(i.into()), *m);
        s.insert(set.position(*m), (i + 1).into());
        if let Set::Role(r) = set {
            s.insert(member_key(r, *m), 1.into());
        }
    }
}
fn base() -> State {
    let mut s = State::new();
    s.insert(0.into(), 1.into());
    replace_set(&mut s, Set::Role(0.into()), &[1.into()]);
    replace_set(&mut s, Set::Role(role("PAUSER_ROLE")), &[2.into()]);
    s.insert(role_admin(role("PAUSER_ROLE")), role("PAUSER_ROLE"));
    replace_set(&mut s, Set::Role(role("UNRELATED_ROLE")), &[88.into()]);
    s.insert(mapping(44.into(), 201.into()), 123.into());
    s.insert(203.into(), 123.into());
    s.insert(mapping(55.into(), mapping(44.into(), 202.into())), 9.into());
    s.insert(mapping(44.into(), 303.into()), 7.into());
    s.insert(404.into(), 1.into());
    s.insert(554.into(), 1000.into());
    s.insert(
        U256::from_str_radix("360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc", 16).unwrap(),
        address(IMPLEMENTATION),
    );
    s
}
fn error(message: &str) -> Vec<u8> {
    let mut data = call("Error(string)", &[32.into(), message.len().into()]);
    data.extend(message.as_bytes());
    data.resize(4 + (data.len() - 4).div_ceil(32) * 32, 0);
    data
}
fn access_error(caller: U256, admin: U256) -> Vec<u8> {
    error(&format!(
        "AccessControl: account 0x{} is missing role {}",
        hex::encode(&vm::word(caller)[12..]),
        w(admin)
    ))
}
fn log_data(values: &[U256]) -> Vec<u8> {
    values.iter().flat_map(|v| vm::word(*v)).collect()
}
type Write = (U256, U256, U256);
#[derive(Clone, Copy)]
enum Route {
    Grant,
    Revoke,
    Renounce,
    Minter(bool),
    Burner(bool),
    Whitelist(bool),
}
fn assert_writes(name: &str, e: &Execution, writes: &[Write], expected: &State) -> Result<()> {
    let actual: Vec<_> = e.writes.iter().map(|s| (s.key, s.old, s.new)).collect();
    ensure!(actual == writes, "{name}: ordered writes differ: actual={actual:?}, expected={writes:?}");
    ensure!(normalize(&e.committed) == normalize(expected), "{name}: exact all-cell state");
    ensure!(e.committed_logs == e.logs, "{name}: committed logs");
    for h in &e.keccaks {
        ensure!(vm::hash(&h.input) == h.output, "{name}: hash witness");
    }
    Ok(())
}
type ExpectedLog = (Vec<U256>, Vec<u8>);
fn assert_attempted(name: &str, e: &Execution, writes: &[Write], logs: &[ExpectedLog], log_after: &[usize]) -> Result<()> {
    ensure!(
        e.writes.iter().map(|s| (s.key, s.old, s.new)).collect::<Vec<_>>() == writes,
        "{name}: complete attempted writes"
    );
    // Validate lengths/content before indexing for interleaving. A missing log
    // must remain an anyhow error so the CLI can preserve its failed report.
    ensure!(
        e.logs.iter().map(|l| (l.topics.clone(), l.data.clone())).collect::<Vec<_>>() == logs,
        "{name}: complete attempted logs"
    );
    ensure!(logs.len() == log_after.len(), "expected log interleave length");
    for (log, index) in e.logs.iter().zip(log_after) {
        let previous = e.writes.get(*index).context("expected preceding store")?;
        ensure!(previous.step < log.step, "{name}: log follows expected store");
        if let Some(next) = e.writes.get(index + 1) {
            ensure!(log.step < next.step, "{name}: log precedes next store");
        }
    }
    Ok(())
}
struct InitializerEffects {
    writes: Vec<Write>,
    logs: Vec<ExpectedLog>,
    log_after: Vec<usize>,
}
fn initializer_effects(owner: U256, pauser: U256, quota: U256, stage: u8) -> InitializerEffects {
    // Pinned Initializable -> ERC20 -> EIP712 -> Pausable parent ordering.
    // Stage 0 stops at the owner/pauser checks; 1 stops at the quota check;
    // stage 2 includes the quota store and final Initializable completion.
    let mut writes = vec![(0.into(), 0.into(), 1.into()), (0.into(), 1.into(), 257.into())];
    for (slot, text) in [
        (204u64, b"BTR token".as_slice()),
        (205, b"BTR".as_slice()),
        (253, b"BTR token".as_slice()),
        (254, b"1".as_slice()),
    ] {
        let mut packed = [0; 32];
        packed[..text.len()].copy_from_slice(text);
        packed[31] = (text.len() * 2) as u8;
        writes.push((slot.into(), 0.into(), U256::from_big_endian(&packed)));
    }
    writes.extend([
        (251.into(), 0.into(), 0.into()),
        (252.into(), 0.into(), 0.into()),
        (404.into(), 0.into(), 0.into()),
    ]);
    let mut logs = vec![];
    let mut log_after = vec![];
    if stage > 0 {
        for (r, member) in [(0.into(), owner), (role("PAUSER_ROLE"), pauser)] {
            writes.push((member_key(r, member), 0.into(), 1.into()));
            logs.push((vec![role("RoleGranted(bytes32,address,address)"), r, member, 9.into()], vec![]));
            log_after.push(writes.len() - 1);
            writes.extend(set_writes(Set::Role(r), &[], member, true).1);
        }
        writes.push((role_admin(role("PAUSER_ROLE")), 0.into(), role("PAUSER_ROLE")));
        logs.push((
            vec![
                role("RoleAdminChanged(bytes32,bytes32,bytes32)"),
                role("PAUSER_ROLE"),
                0.into(),
                role("PAUSER_ROLE"),
            ],
            vec![],
        ));
        log_after.push(writes.len() - 1);
        writes.push((404.into(), 0.into(), 1.into()));
        logs.push((vec![role("Paused(address)")], vm::word(9.into()).to_vec()));
        log_after.push(writes.len() - 1);
        if stage == 2 {
            writes.extend([(554.into(), 0.into(), quota), (0.into(), 257.into(), 1.into())]);
            logs.push((vec![role("Initialized(uint8)")], vm::word(1.into()).to_vec()));
            log_after.push(writes.len() - 1);
        }
    }
    InitializerEffects { writes, logs, log_after }
}
fn apply(expected: &mut State, writes: &[Write]) -> Result<()> {
    for (key, old, new) in writes {
        ensure!(get(expected, *key) == *old, "independent expected old value");
        expected.insert(*key, *new);
    }
    Ok(())
}
fn set_writes(set: Set, before: &[U256], member: U256, grant: bool) -> (Vec<U256>, Vec<Write>) {
    let mut after = before.to_vec();
    let mut writes = vec![];
    let index = before.iter().position(|v| *v == member);
    if grant && index.is_none() {
        after.push(member);
        writes.extend([
            (set.head(), before.len().into(), after.len().into()),
            (set.element(before.len().into()), 0.into(), member),
            (set.position(member), 0.into(), after.len().into()),
        ]);
    } else if !grant {
        if let Some(i) = index {
            let tail = before[before.len() - 1];
            after.swap_remove(i);
            if i != before.len() - 1 {
                writes.extend([(set.element(i.into()), member, tail), (set.position(tail), before.len().into(), (i + 1).into())]);
            }
            writes.extend([
                (set.element((before.len() - 1).into()), tail, 0.into()),
                (set.head(), before.len().into(), after.len().into()),
                (set.position(member), (i + 1).into(), 0.into()),
            ]);
        }
    }
    (after, writes)
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
        signature: &str,
        code: &[u8],
        data: &[u8],
        caller: U256,
        account: U256,
        size: usize,
        pre: &State,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<Execution> {
        let e = vm::execute_with_self_code_size(code, data, caller, account, pre, Some(size));
        self.calls += 1;
        let record = json!({"name":name,"signature":signature,"caller":w(caller),"address":w(account),"self_code_size":size,"calldata":hex::encode(data),"prestate":state_json(pre),"execution":execution_json(&e)});
        save(self.calls, &record)?;
        self.cases.push(record);
        ensure!(!matches!(e.exit, Exit::HarnessFailure(_) | Exit::Invalid), "{name}: {:?}", e.exit);
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
        self.record(
            name,
            sig,
            self.runtime,
            &call(sig, args),
            caller,
            address(PROXY),
            CAPTURES[1].runtime_bytes,
            pre,
            save,
        )
    }
    pub fn constructor(&mut self, creation: &[u8], save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let e = self.record(
            "implementation_constructor",
            "constructor()",
            creation,
            &[],
            9.into(),
            address(IMPLEMENTATION),
            0,
            &State::new(),
            save,
        )?;
        ensure!(e.exit == Exit::Return(self.runtime.to_vec()), "exact constructor runtime: {:?}", e.exit);
        let expected = State::from([(0.into(), 255.into())]);
        assert_writes("implementation_constructor", &e, &[(0.into(), 0.into(), 255.into())], &expected)?;
        ensure!(
            e.logs.len() == 1 && e.logs[0].topics == vec![role("Initialized(uint8)")] && e.logs[0].data == vm::word(255.into()),
            "constructor initialized log"
        );
        Ok(())
    }
    fn getter(
        &mut self,
        name: &str,
        sig: &str,
        args: &[U256],
        expected: Vec<u8>,
        state: &State,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<()> {
        let e = self.run(name, sig, args, 9.into(), state, save)?;
        ensure!(e.exit == Exit::Return(expected), "{name}: getter {:?}", e.exit);
        ensure!(e.writes.is_empty() && e.logs.is_empty() && e.committed == *state, "{name}: getter mutated");
        Ok(())
    }
    fn verify_set(
        &mut self,
        name: &str,
        set: Set,
        members: &[U256],
        checked: U256,
        state: &State,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<()> {
        for (i, m) in members.iter().enumerate() {
            ensure!(
                get(state, set.element(i.into())) == *m && get(state, set.position(*m)) == (i + 1).into(),
                "{name}: set coherence"
            );
        }
        match set {
            Set::Role(r) => {
                self.getter(name, "getRoleMemberCount(bytes32)", &[r], vm::word(members.len().into()).to_vec(), state, save)?;
                self.getter(name, "getRoleAdmin(bytes32)", &[r], vm::word(get(state, role_admin(r))).to_vec(), state, save)?;
                for (i, m) in members.iter().enumerate() {
                    self.getter(name, "getRoleMember(bytes32,uint256)", &[r, i.into()], vm::word(*m).to_vec(), state, save)?;
                }
                for m in members
                    .iter()
                    .copied()
                    .chain([checked, 0.into(), 99.into()])
                    .collect::<std::collections::BTreeSet<_>>()
                {
                    self.getter(
                        name,
                        "hasRole(bytes32,address)",
                        &[r, m],
                        vm::word(u8::from(members.contains(&m)).into()).to_vec(),
                        state,
                        save,
                    )?;
                }
                let e = self.run(name, "getRoleMember(bytes32,uint256)", &[r, members.len().into()], 9.into(), state, save)?;
                Self::reverted(name, &e, state, call("Panic(uint256)", &[0x32.into()]))?;
            }
            Set::Whitelist => {
                let mut want = log_data(&[32.into(), members.len().into()]);
                want.extend(log_data(members));
                self.getter(name, "queryWhitelisted()", &[], want, state, save)?;
            }
        }
        Ok(())
    }
    fn preserved_balances(&mut self, name: &str, state: &State, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        for (sig, args, want) in [
            ("balanceOf(address)", vec![44.into()], 123),
            ("totalSupply()", vec![], 123),
            ("allowance(address,address)", vec![44.into(), 55.into()], 9),
            ("nonces(address)", vec![44.into()], 7),
        ] {
            self.getter(name, sig, &args, vm::word(U256::from(want as u64)).to_vec(), state, save)?;
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn operation(
        &mut self,
        name: &str,
        set: Set,
        before: &[U256],
        member: U256,
        route: Route,
        previous: Option<State>,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<State> {
        let continuing = previous.is_some();
        let mut pre = previous.unwrap_or_else(base);
        if continuing {
            ensure!(get(&pre, set.head()) == before.len().into(), "sequence actual length");
            for (i, m) in before.iter().enumerate() {
                ensure!(
                    get(&pre, set.element(i.into())) == *m && get(&pre, set.position(*m)) == (i + 1).into(),
                    "sequence actual cells"
                );
            }
        } else {
            replace_set(&mut pre, set, before);
        }
        let r = match set {
            Set::Role(r) => r,
            Set::Whitelist => 0.into(),
        };
        let grant = matches!(route, Route::Grant | Route::Minter(true) | Route::Burner(true) | Route::Whitelist(true));
        let (sig, args, caller, wrapper) = match route {
            Route::Grant | Route::Revoke => (
                if grant { "grantRole(bytes32,address)" } else { "revokeRole(bytes32,address)" },
                vec![r, member],
                if r == role("PAUSER_ROLE") { 2.into() } else { 1.into() },
                None,
            ),
            Route::Renounce => ("renounceRole(bytes32,address)", vec![r, member], member, None),
            Route::Minter(on) => (
                "setMinter(address,bool)",
                vec![member, u8::from(on).into()],
                1.into(),
                Some("MinterSet(address,bool)"),
            ),
            Route::Burner(on) => (
                "setBurner(address,bool)",
                vec![member, u8::from(on).into()],
                1.into(),
                Some("BurnerSet(address,bool)"),
            ),
            Route::Whitelist(on) => (
                "setWhitelister(address,bool)",
                vec![member, u8::from(on).into()],
                2.into(),
                Some("WhitelistedSet(address,bool)"),
            ),
        };
        let e = self.run(name, sig, &args, caller, &pre, save)?;
        ensure!(e.exit == Exit::Return(vec![]), "{name}: success {:?}", e.exit);
        let (after, set_effects) = set_writes(set, before, member, grant);
        let changed = before.contains(&member) != grant;
        let mut writes = vec![];
        let role_changed = matches!(set, Set::Role(_)) && changed;
        if role_changed {
            writes.push((member_key(r, member), u8::from(!grant).into(), u8::from(grant).into()));
        }
        writes.extend(set_effects);
        let mut expected = pre.clone();
        apply(&mut expected, &writes)?;
        assert_writes(name, &e, &writes, &expected)?;
        let mut logs: Vec<(Vec<U256>, Vec<u8>)> = vec![];
        if role_changed {
            logs.push((
                vec![
                    role(if grant {
                        "RoleGranted(bytes32,address,address)"
                    } else {
                        "RoleRevoked(bytes32,address,address)"
                    }),
                    r,
                    member,
                    caller,
                ],
                vec![],
            ));
        }
        if let Some(event) = wrapper {
            logs.push((vec![role(event)], log_data(&[member, u8::from(grant).into()])));
        }
        ensure!(
            e.logs.iter().map(|l| (l.topics.clone(), l.data.clone())).collect::<Vec<_>>() == logs,
            "{name}: complete event sequence"
        );
        if role_changed {
            ensure!(
                e.writes[0].step < e.logs[0].step && e.logs[0].step < e.writes[1].step,
                "{name}: bool/log/set order"
            );
        }
        if wrapper.is_some() && !e.writes.is_empty() {
            ensure!(e.writes.last().unwrap().step < e.logs.last().unwrap().step, "wrapper log follows all writes");
        }
        // No-op paths still read the selected position and, for roles, the
        // membership boolean. OZ 4 must not be mistaken for a bool-only check.
        let mut required_reads = vec![set.position(member)];
        if let Set::Role(_) = set {
            required_reads.push(member_key(r, member));
        }
        match route {
            Route::Grant | Route::Revoke => {
                required_reads.push(role_admin(r));
                required_reads.push(member_key(get(&pre, role_admin(r)), caller));
            }
            Route::Minter(_) | Route::Burner(_) => required_reads.push(member_key(0.into(), caller)),
            Route::Whitelist(_) => required_reads.push(member_key(role("PAUSER_ROLE"), caller)),
            Route::Renounce => {}
        }
        for key in required_reads {
            ensure!(
                e.reads.iter().any(|read| read.key == key && read.value == get(&pre, key)),
                "{name}: required authorization/member/position read"
            );
        }
        if changed {
            for input in [vm::word(set.head()).to_vec(), log_data(&[member, set.head().overflowing_add(1.into()).0])] {
                ensure!(e.keccaks.iter().any(|h| h.input == input), "{name}: actual set preimage");
            }
            if let Set::Role(r) = set {
                for input in [
                    log_data(&[r, 101.into()]),
                    log_data(&[member, mapping(r, 101.into())]),
                    log_data(&[r, 151.into()]),
                ] {
                    ensure!(e.keccaks.iter().any(|h| h.input == input), "{name}: linked role preimage");
                }
            }
        }
        self.verify_set(name, set, &after, member, &e.committed, save)?;
        self.preserved_balances(name, &e.committed, save)?;
        let owners = Set::Role(0.into());
        if get(&e.committed, owners.head()).is_zero() {
            let owner = self.run(name, "owner()", &[], 9.into(), &e.committed, save)?;
            Self::reverted(name, &owner, &e.committed, call("Panic(uint256)", &[0x32.into()]))?;
        } else {
            self.getter(
                name,
                "owner()",
                &[],
                vm::word(get(&e.committed, owners.element(0.into()))).to_vec(),
                &e.committed,
                save,
            )?;
        }
        Ok(e.committed)
    }
    fn reverted(name: &str, e: &Execution, pre: &State, data: Vec<u8>) -> Result<()> {
        ensure!(e.exit == Exit::Revert(data), "{name}: exact revert {:?}", e.exit);
        ensure!(e.committed == *pre && e.committed_logs.is_empty(), "{name}: complete rollback");
        Ok(())
    }
    pub fn matrix(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let r = role("MATRIX_ROLE");
        for (name, before, member, grant) in [
            ("add_empty", vec![], 3, true),
            ("add_nonempty", vec![3, 4], 5, true),
            ("add_zero_empty", vec![], 0, true),
            ("add_zero_nonempty", vec![3, 4], 0, true),
            ("duplicate", vec![3, 4], 3, true),
            ("duplicate_zero", vec![0, 3], 0, true),
            ("remove_first", vec![3, 4, 5], 3, false),
            ("remove_middle", vec![3, 4, 5], 4, false),
            ("remove_tail", vec![3, 4, 5], 5, false),
            ("remove_sole", vec![3], 3, false),
            ("remove_zero_tail_swap", vec![3, 4, 0], 4, false),
            ("remove_zero_first", vec![0, 3, 4], 0, false),
            ("remove_zero_middle", vec![3, 0, 4], 0, false),
            ("remove_zero_tail", vec![3, 4, 0], 0, false),
            ("remove_zero_sole", vec![0], 0, false),
            ("absent_empty", vec![], 3, false),
            ("absent_nonempty", vec![3, 4], 5, false),
        ] {
            let before: Vec<_> = before.into_iter().map(|v| U256::from(v as u64)).collect();
            let member = U256::from(member as u64);
            self.operation(
                &format!("role_{name}"),
                Set::Role(r),
                &before,
                member,
                if grant { Route::Grant } else { Route::Revoke },
                None,
                save,
            )?;
            self.operation(
                &format!("whitelist_{name}"),
                Set::Whitelist,
                &before,
                member,
                Route::Whitelist(grant),
                None,
                save,
            )?;
        }
        for (name, r, before) in [
            ("default_admin", U256::zero(), vec![1.into()]),
            ("pauser", role("PAUSER_ROLE"), vec![2.into()]),
            ("minter", role("MINTER_ROLE"), vec![]),
            ("burner", role("BURNER_ROLE"), vec![]),
            ("arbitrary", U256::max_value(), vec![]),
        ] {
            self.operation(name, Set::Role(r), &before, (U256::one() << 160) - U256::one(), Route::Grant, None, save)?;
        }
        for (name, before, member) in [
            ("renounce_present", vec![3.into(), 4.into()], 3.into()),
            ("renounce_absent", vec![4.into()], 3.into()),
            ("renounce_zero", vec![0.into()], 0.into()),
            ("renounce_last_owner", vec![1.into()], 1.into()),
        ] {
            self.operation(
                name,
                Set::Role(if name == "renounce_last_owner" { 0.into() } else { r }),
                &before,
                member,
                Route::Renounce,
                None,
                save,
            )?;
        }
        for (label, role_name, minter) in [("minter", "MINTER_ROLE", true), ("burner", "BURNER_ROLE", false)] {
            for (suffix, before, enabled) in [
                ("add", vec![], true),
                ("duplicate", vec![3.into()], true),
                ("remove", vec![3.into(), 4.into()], false),
                ("absent", vec![], false),
            ] {
                self.operation(
                    &format!("wrapper_{label}_{suffix}"),
                    Set::Role(role(role_name)),
                    &before,
                    3.into(),
                    if minter { Route::Minter(enabled) } else { Route::Burner(enabled) },
                    None,
                    save,
                )?;
            }
        }
        self.failures(save)?;
        self.incoherent(save)?;
        self.sequences(save)?;
        self.initializers(save)?;
        self.balance_controls(save)?;
        Ok(())
    }
    fn failures(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let r = role("MATRIX_ROLE");
        for (name, sig, args, caller, expected) in [
            (
                "unauthorized_grant",
                "grantRole(bytes32,address)",
                vec![r, 3.into()],
                9,
                access_error(9.into(), 0.into()),
            ),
            (
                "unauthorized_revoke",
                "revokeRole(bytes32,address)",
                vec![r, 3.into()],
                9,
                access_error(9.into(), 0.into()),
            ),
            (
                "owner_cannot_grant_pauser",
                "grantRole(bytes32,address)",
                vec![role("PAUSER_ROLE"), 3.into()],
                1,
                access_error(1.into(), role("PAUSER_ROLE")),
            ),
            (
                "owner_cannot_whitelist",
                "setWhitelister(address,bool)",
                vec![3.into(), 1.into()],
                1,
                access_error(1.into(), role("PAUSER_ROLE")),
            ),
            (
                "pauser_cannot_set_minter",
                "setMinter(address,bool)",
                vec![3.into(), 1.into()],
                2,
                access_error(2.into(), 0.into()),
            ),
            (
                "pauser_cannot_set_burner",
                "setBurner(address,bool)",
                vec![3.into(), 0.into()],
                2,
                access_error(2.into(), 0.into()),
            ),
            (
                "renounce_wrong_caller",
                "renounceRole(bytes32,address)",
                vec![r, 3.into()],
                9,
                error("AccessControl: can only renounce roles for self"),
            ),
        ] {
            let pre = base();
            let e = self.run(name, sig, &args, U256::from(caller as u64), &pre, save)?;
            Self::reverted(name, &e, &pre, expected)?;
            ensure!(e.writes.is_empty() && e.logs.is_empty(), "authorization before effects");
        }
        for (name, mut data) in [
            ("noncanonical_address", call("grantRole(bytes32,address)", &[r, U256::one() << 160])),
            ("noncanonical_bool", call("setWhitelister(address,bool)", &[3.into(), 2.into()])),
            ("truncated_calldata", call("grantRole(bytes32,address)", &[r])),
            ("unknown_selector", vec![0xff; 4]),
        ] {
            if name == "truncated_calldata" {
                data.truncate(20);
            }
            let pre = base();
            let e = self.record(name, "malformed ABI", self.runtime, &data, 1.into(), address(PROXY), 183, &pre, save)?;
            Self::reverted(name, &e, &pre, vec![])?;
            ensure!(e.writes.is_empty() && e.logs.is_empty(), "ABI before effects");
        }
        Ok(())
    }
    fn incoherent(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let r = role("INCOHERENT_ROLE");
        let set = Set::Role(r);
        for (name, bool_before, indexed, grant) in [
            ("incoherent_set_only_grant", true, false, true),
            ("incoherent_bool_only_grant", false, true, true),
            ("incoherent_bool_only_revoke", true, false, false),
            ("incoherent_set_only_revoke", false, true, false),
        ] {
            let mut pre = base();
            let members = if indexed { vec![3.into(), 4.into()] } else { vec![] };
            replace_set(&mut pre, set, &members);
            pre.insert(member_key(r, 3.into()), u8::from(bool_before).into());
            let e = self.run(
                name,
                if grant { "grantRole(bytes32,address)" } else { "revokeRole(bytes32,address)" },
                &[r, 3.into()],
                1.into(),
                &pre,
                save,
            )?;
            ensure!(e.exit == Exit::Return(vec![]), "{name}: explicit out-of-domain success");
            let (_, set_effects) = set_writes(set, &members, 3.into(), grant);
            let mut writes = vec![];
            if bool_before != grant {
                writes.push((member_key(r, 3.into()), u8::from(bool_before).into(), u8::from(grant).into()));
            }
            writes.extend(set_effects);
            let mut expected = pre.clone();
            apply(&mut expected, &writes)?;
            assert_writes(name, &e, &writes, &expected)?;
            if bool_before != grant {
                ensure!(
                    e.logs.len() == 1
                        && e.logs[0].topics
                            == vec![
                                role(if grant {
                                    "RoleGranted(bytes32,address,address)"
                                } else {
                                    "RoleRevoked(bytes32,address,address)"
                                }),
                                r,
                                3.into(),
                                1.into()
                            ]
                        && e.logs[0].data.is_empty(),
                    "conditional bool log"
                );
            } else {
                ensure!(e.logs.is_empty(), "set-only operation has no role log");
            }
        }
        for (name, index, len, panic) in [
            ("empty_set_prefix_revert", U256::one(), 0u64, 0x11),
            ("index_out_of_bounds_prefix_revert", 3.into(), 1, 0x32),
            ("max_index_prefix_revert", U256::max_value(), 1, 0x32),
        ] {
            let mut pre = base();
            pre.insert(member_key(r, 3.into()), 1.into());
            pre.insert(set.position(3.into()), index);
            pre.insert(set.head(), len.into());
            pre.insert(set.element(0.into()), 4.into());
            let e = self.run(name, "revokeRole(bytes32,address)", &[r, 3.into()], 1.into(), &pre, save)?;
            Self::reverted(name, &e, &pre, call("Panic(uint256)", &[U256::from(panic as u64)]))?;
            assert_attempted(
                name,
                &e,
                &[(member_key(r, 3.into()), 1.into(), 0.into())],
                &[(vec![role("RoleRevoked(bytes32,address,address)"), r, 3.into(), 1.into()], vec![])],
                &[0],
            )?;
        }
        // The overflow result must be confirmed by execution of this compiler;
        // no coherent set is represented by this synthetic maximum-length state.
        let mut pre = base();
        pre.insert(set.head(), U256::max_value());
        let e = self.run("incoherent_max_length_push", "grantRole(bytes32,address)", &[r, 3.into()], 1.into(), &pre, save)?;
        ensure!(e.exit == Exit::Return(vec![]), "maximum array length outcome {:?}", e.exit);
        let writes = [
            (member_key(r, 3.into()), 0.into(), 1.into()),
            (set.head(), U256::max_value(), 0.into()),
            (set.element(U256::max_value()), 0.into(), 3.into()),
            (set.position(3.into()), 0.into(), 0.into()),
        ];
        let mut expected = pre.clone();
        apply(&mut expected, &writes)?;
        assert_writes("incoherent_max_length_push", &e, &writes, &expected)?;
        ensure!(e.logs.len() == 1, "maximum length bool log");
        Ok(())
    }
    fn sequences(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let mut state = base();
        let r = role("SEQUENCE_ROLE");
        let mut members = vec![];
        let mut whitelist = vec![];
        for (i, (grant, member)) in [(true, 3u64), (true, 0), (true, 4), (false, 3), (true, 3), (false, 0), (false, 4), (false, 3)]
            .into_iter()
            .enumerate()
        {
            let member = member.into();
            state = self.operation(
                &format!("sequence_role_{i}"),
                Set::Role(r),
                &members,
                member,
                if grant { Route::Grant } else { Route::Revoke },
                Some(state),
                save,
            )?;
            members = set_writes(Set::Role(r), &members, member, grant).0;
            state = self.operation(
                &format!("sequence_whitelist_{i}"),
                Set::Whitelist,
                &whitelist,
                member,
                Route::Whitelist(grant),
                Some(state),
                save,
            )?;
            whitelist = set_writes(Set::Whitelist, &whitelist, member, grant).0;
        }
        Ok(())
    }
    fn initializers(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let max = U256::from(1_000_000_000u64) * U256::exp10(18);
        let captured_owner = address("0x27e939c9a85afd8d643eeda0bdecb193683bcda5");
        for (name, owner, pauser, quota) in [
            ("initialize_captured_arguments", captured_owner, captured_owner, max / 2),
            ("initialize_distinct_owner_pauser", 1.into(), 2.into(), 1000.into()),
            ("initialize_zero_quota", 1.into(), 2.into(), 0.into()),
        ] {
            let mut data = super::initializer_arguments();
            for (index, value) in [(2, owner), (3, pauser), (4, quota)] {
                data[4 + index * 32..4 + (index + 1) * 32].copy_from_slice(&vm::word(value));
            }
            let pre = State::new();
            let e = self.record(
                name,
                "initialize(string,string,address,address,uint256)",
                self.runtime,
                &data,
                9.into(),
                address(PROXY),
                183,
                &pre,
                save,
            )?;
            ensure!(e.exit == Exit::Return(vec![]), "{name}: success {:?}", e.exit);
            let effects = initializer_effects(owner, pauser, quota, 2);
            assert_attempted(name, &e, &effects.writes, &effects.logs, &effects.log_after)?;
            let mut expected = State::new();
            expected.insert(0.into(), 1.into());
            for (slot, text) in [
                (204, b"BTR token".as_slice()),
                (205, b"BTR".as_slice()),
                (253, b"BTR token".as_slice()),
                (254, b"1".as_slice()),
            ] {
                let mut packed = [0; 32];
                packed[..text.len()].copy_from_slice(text);
                packed[31] = (text.len() * 2) as u8;
                expected.insert(U256::from(slot as u64), U256::from_big_endian(&packed));
            }
            replace_set(&mut expected, Set::Role(0.into()), &[owner]);
            replace_set(&mut expected, Set::Role(role("PAUSER_ROLE")), &[pauser]);
            expected.insert(role_admin(role("PAUSER_ROLE")), role("PAUSER_ROLE"));
            expected.insert(404.into(), 1.into());
            expected.insert(554.into(), quota);
            ensure!(normalize(&e.committed) == normalize(&expected), "{name}: full initialized storage");
            let logs = vec![
                (vec![role("RoleGranted(bytes32,address,address)"), 0.into(), owner, 9.into()], vec![]),
                (
                    vec![role("RoleGranted(bytes32,address,address)"), role("PAUSER_ROLE"), pauser, 9.into()],
                    vec![],
                ),
                (
                    vec![
                        role("RoleAdminChanged(bytes32,bytes32,bytes32)"),
                        role("PAUSER_ROLE"),
                        0.into(),
                        role("PAUSER_ROLE"),
                    ],
                    vec![],
                ),
                (vec![role("Paused(address)")], vm::word(9.into()).to_vec()),
                (vec![role("Initialized(uint8)")], vm::word(1.into()).to_vec()),
            ];
            ensure!(
                e.logs.iter().map(|l| (l.topics.clone(), l.data.clone())).collect::<Vec<_>>() == logs && e.committed_logs == e.logs,
                "{name}: exact initializer log order"
            );
            for (r, member, log_index) in [(0.into(), owner, 0usize), (role("PAUSER_ROLE"), pauser, 1)] {
                let b = e.writes.iter().find(|s| s.key == member_key(r, member)).context("initializer bool")?;
                let len = e.writes.iter().find(|s| s.key == Set::Role(r).head()).context("initializer length")?;
                ensure!(
                    b.step < e.logs[log_index].step && e.logs[log_index].step < len.step,
                    "initializer bool/log/set ordering"
                );
                self.verify_set(name, Set::Role(r), &[member], 3.into(), &e.committed, save)?;
            }
            for (sig, args, want) in [
                ("owner()", vec![], owner),
                ("paused()", vec![], 1.into()),
                ("mintQuota()", vec![], quota),
                ("totalSupply()", vec![], 0.into()),
                ("balanceOf(address)", vec![owner], 0.into()),
            ] {
                self.getter(name, sig, &args, vm::word(want).to_vec(), &e.committed, save)?;
            }
            for (sig, text) in [("name()", b"BTR token".as_slice()), ("symbol()", b"BTR".as_slice())] {
                let mut want = log_data(&[32.into(), text.len().into()]);
                want.extend(text);
                want.resize(want.len().div_ceil(32) * 32, 0);
                self.getter(name, sig, &[], want, &e.committed, save)?;
            }
            self.verify_set(name, Set::Whitelist, &[], 3.into(), &e.committed, save)?;
            let again = self.record(
                "repeat_proxy_initializer",
                "initialize(string,string,address,address,uint256)",
                self.runtime,
                &data,
                9.into(),
                address(PROXY),
                183,
                &e.committed,
                save,
            )?;
            Self::reverted(name, &again, &e.committed, error("Initializable: contract is already initialized"))?;
            ensure!(again.writes.is_empty() && again.logs.is_empty(), "repeat initializer before effects");
            if owner != pauser {
                let denied = self.run(
                    "initialized_owner_cannot_whitelist",
                    "setWhitelister(address,bool)",
                    &[3.into(), 1.into()],
                    owner,
                    &e.committed,
                    save,
                )?;
                Self::reverted(name, &denied, &e.committed, access_error(owner, role("PAUSER_ROLE")))?;
                let allowed = self.run(
                    "initialized_pauser_can_whitelist",
                    "setWhitelister(address,bool)",
                    &[3.into(), 1.into()],
                    pauser,
                    &e.committed,
                    save,
                )?;
                let (_, writes) = set_writes(Set::Whitelist, &[], 3.into(), true);
                let mut expected = e.committed.clone();
                apply(&mut expected, &writes)?;
                assert_writes(name, &allowed, &writes, &expected)?;
                ensure!(
                    allowed.exit == Exit::Return(vec![])
                        && allowed.logs.len() == 1
                        && allowed.logs[0].topics == vec![role("WhitelistedSet(address,bool)")]
                        && allowed.logs[0].data == log_data(&[3.into(), 1.into()]),
                    "initialized pauser whitelist"
                );
            }
        }
        for (name, index, value, message) in [
            ("initialize_zero_owner", 2, U256::zero(), "owner cannot be zero address"),
            ("initialize_zero_pauser", 3, U256::zero(), "pauser cannot be zero address"),
            ("initialize_quota_overflow", 4, max + 1, "mintQuota exceeds max supply"),
        ] {
            let mut data = super::initializer_arguments();
            data[4 + index * 32..4 + (index + 1) * 32].copy_from_slice(&vm::word(value));
            let pre = State::new();
            let e = self.record(
                name,
                "initialize(string,string,address,address,uint256)",
                self.runtime,
                &data,
                9.into(),
                address(PROXY),
                183,
                &pre,
                save,
            )?;
            Self::reverted(name, &e, &pre, error(message))?;
            let effects = initializer_effects(captured_owner, captured_owner, value, if index == 4 { 1 } else { 0 });
            assert_attempted(name, &e, &effects.writes, &effects.logs, &effects.log_after)?;
        }
        let pre = State::from([(0.into(), 255.into())]);
        let e = self.record(
            "locked_implementation_initializer",
            "initialize(string,string,address,address,uint256)",
            self.runtime,
            &super::initializer_arguments(),
            9.into(),
            address(IMPLEMENTATION),
            15308,
            &pre,
            save,
        )?;
        Self::reverted(
            "locked_implementation_initializer",
            &e,
            &pre,
            error("Initializable: contract is already initialized"),
        )?;
        ensure!(e.writes.is_empty() && e.logs.is_empty(), "implementation locked before effects");
        Ok(())
    }
    fn balance_controls(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        for value in [U256::zero(), U256::one(), 123.into(), U256::max_value()] {
            let mut pre = base();
            pre.insert(mapping(44.into(), 201.into()), value);
            pre.insert(203.into(), value);
            for paused in [0, 1] {
                pre.insert(404.into(), U256::from(paused as u64));
                let whitelisted = [44.into()];
                replace_set(&mut pre, Set::Whitelist, if paused == 0 { &whitelisted } else { &[] });
                self.getter(
                    "raw_balance_independent_of_pause_whitelist",
                    "balanceOf(address)",
                    &[44.into()],
                    vm::word(value).to_vec(),
                    &pre,
                    save,
                )?;
                self.getter("raw_supply", "totalSupply()", &[], vm::word(value).to_vec(), &pre, save)?;
                self.getter(
                    "unobserved_synthetic_holder",
                    "balanceOf(address)",
                    &[45.into()],
                    vm::word(0.into()).to_vec(),
                    &pre,
                    save,
                )?;
            }
        }
        let mut state = base();
        replace_set(&mut state, Set::Role(role("MINTER_ROLE")), &[1.into()]);
        replace_set(&mut state, Set::Role(role("BURNER_ROLE")), &[1.into()]);
        let blocked = self.run(
            "paused_unlisted_sender",
            "transfer(address,uint256)",
            &[45.into(), 20.into()],
            44.into(),
            &state,
            save,
        )?;
        Self::reverted("paused_unlisted_sender", &blocked, &state, error("paused and not whitelisted"))?;
        ensure!(blocked.writes.is_empty() && blocked.logs.is_empty(), "blocked transfer before balances");
        let mut recipient_only = state.clone();
        replace_set(&mut recipient_only, Set::Whitelist, &[45.into()]);
        let blocked = self.run(
            "paused_recipient_whitelist_insufficient",
            "transfer(address,uint256)",
            &[45.into(), 20.into()],
            44.into(),
            &recipient_only,
            save,
        )?;
        Self::reverted(
            "paused_recipient_whitelist_insufficient",
            &blocked,
            &recipient_only,
            error("paused and not whitelisted"),
        )?;
        replace_set(&mut state, Set::Whitelist, &[44.into()]);
        for (name, sig, args, caller, from, to, amount, supply, sender, recipient) in [
            (
                "paused_whitelisted_transfer",
                "transfer(address,uint256)",
                vec![45.into(), 20.into()],
                44u64,
                44u64,
                45u64,
                20u64,
                123u64,
                103u64,
                20u64,
            ),
            (
                "paused_mint_exempt",
                "mint(address,uint256)",
                vec![45.into(), 10.into()],
                1,
                0,
                45,
                10,
                133,
                103,
                30,
            ),
            (
                "paused_admin_burn_exempt",
                "burn(address,uint256)",
                vec![45.into(), 5.into()],
                1,
                45,
                0,
                5,
                128,
                103,
                25,
            ),
            ("paused_self_burn_exempt", "burn(uint256)", vec![5.into()], 45, 45, 0, 5, 123, 103, 20),
        ] {
            let e = self.run(name, sig, &args, caller.into(), &state, save)?;
            ensure!(
                e.exit == Exit::Return(if from == 44 { vm::word(1.into()).to_vec() } else { vec![] }),
                "{name}: return"
            );
            let mut expected = state.clone();
            expected.insert(203.into(), supply.into());
            expected.insert(mapping(44.into(), 201.into()), sender.into());
            expected.insert(mapping(45.into(), 201.into()), recipient.into());
            ensure!(normalize(&e.committed) == normalize(&expected), "{name}: all-cell state");
            ensure!(
                e.logs.len() == 1
                    && e.logs[0].topics == vec![role("Transfer(address,address,uint256)"), from.into(), to.into()]
                    && e.logs[0].data == vm::word(amount.into())
                    && e.committed_logs == e.logs,
                "{name}: transfer log"
            );
            state = e.committed;
            for (sig, args, want) in [
                ("balanceOf(address)", vec![44.into()], sender),
                ("balanceOf(address)", vec![45.into()], recipient),
                ("totalSupply()", vec![], supply),
            ] {
                self.getter(name, sig, &args, vm::word(want.into()).to_vec(), &state, save)?;
            }
        }
        replace_set(&mut state, Set::Whitelist, &[]);
        let e = self.run("unpause", "unpause()", &[], 2.into(), &state, save)?;
        let mut expected = state.clone();
        expected.insert(404.into(), 0.into());
        assert_writes("unpause", &e, &[(404.into(), 1.into(), 0.into())], &expected)?;
        ensure!(
            e.exit == Exit::Return(vec![]) && e.logs.len() == 1 && e.logs[0].topics == vec![role("Unpaused(address)")] && e.logs[0].data == vm::word(2.into()),
            "unpause log"
        );
        state = e.committed;
        let e = self.run(
            "unpaused_unlisted_transfer",
            "transfer(address,uint256)",
            &[45.into(), 1.into()],
            44.into(),
            &state,
            save,
        )?;
        let mut expected = state.clone();
        expected.insert(mapping(44.into(), 201.into()), 102.into());
        expected.insert(mapping(45.into(), 201.into()), 21.into());
        ensure!(
            e.exit == Exit::Return(vm::word(1.into()).to_vec()) && normalize(&e.committed) == normalize(&expected),
            "unpaused transfer exact state"
        );
        ensure!(
            e.logs.len() == 1
                && e.logs[0].topics == vec![role("Transfer(address,address,uint256)"), 44.into(), 45.into()]
                && e.logs[0].data == vm::word(1.into())
                && e.committed_logs == e.logs,
            "unpaused transfer log"
        );
        Ok(())
    }
}

#[cfg(test)]
mod validation_tests {
    use super::*;
    #[test]
    fn btr_missing_role_log_returns_error_after_preserving_the_attempt() {
        // Deliberately broken local bytecode produces the expected four stores
        // but omits the role log. This is a validator robustness control, not
        // execution of the selected BTR runtime or a source-agreement case.
        let r = role("OMITTED_LOG_CONTROL");
        let set = Set::Role(r);
        let mut code = vec![];
        for (key, value) in [
            (member_key(r, 3.into()), 1.into()),
            (set.head(), 1.into()),
            (set.element(0.into()), 3.into()),
            (set.position(3.into()), 1.into()),
        ] {
            code.push(0x7f);
            code.extend(vm::word(value));
            code.push(0x7f);
            code.extend(vm::word(key));
            code.push(0x55);
        }
        code.push(0x00);
        let mut proof = Proof::new(&code);
        let mut saved = vec![];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            proof.operation("omitted_role_log", set, &[], 3.into(), Route::Grant, None, &mut |_, v| {
                saved.push(v.clone());
                Ok(())
            })
        }));
        assert!(result.is_ok(), "missing log must return a reportable error instead of panicking");
        assert!(result.unwrap().is_err());
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0]["execution"]["writes"].as_array().unwrap().len(), 4);
        assert!(saved[0]["execution"]["logs"].as_array().unwrap().is_empty());
    }
}
