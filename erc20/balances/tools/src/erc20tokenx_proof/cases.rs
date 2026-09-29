//! Selected ERC20TokenX bytecode in synthetic local state, never admission.
use super::{address, bytes, vm, FNA, ORI, PHI};
use crate::ptoken_proof::cases::{call, execution_json, get, mapping, role, state_json, w};
use anyhow::{ensure, Context, Result};
use primitive_types::U256;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use vm::{Execution, Exit, State};

pub fn head(r: U256) -> U256 {
    mapping(r, 8.into())
}
pub fn admin(r: U256) -> U256 {
    head(r).overflowing_add(2.into()).0
}
pub fn element(r: U256, i: U256) -> U256 {
    vm::hash(&vm::word(head(r))).overflowing_add(i).0
}
pub fn position(r: U256, m: U256) -> U256 {
    mapping(m, head(r).overflowing_add(1.into()).0)
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
fn seed(s: &mut State, r: U256, members: &[U256]) {
    assert!(get(s, head(r)) < 32.into());
    for i in 0..get(s, head(r)).as_usize() {
        let m = get(s, element(r, i.into()));
        s.remove(&element(r, i.into()));
        s.remove(&position(r, m));
    }
    s.insert(head(r), members.len().into());
    for (i, m) in members.iter().enumerate() {
        s.insert(element(r, i.into()), *m);
        s.insert(position(r, *m), (i + 1).into());
    }
}
fn base() -> State {
    let mut s = State::new();
    seed(&mut s, 0.into(), &[1.into()]);
    seed(&mut s, role("SENTINEL_ROLE"), &[88.into()]);
    for (k, v) in [
        (2, 123.into()),
        (3, packed("Synthetic")),
        (4, packed("SYN")),
        (5, 9.into()),
        (7, role("DOMAIN_SENTINEL")),
        (9, 77.into()),
        (10, 78.into()),
        (11, 1000.into()),
        (12, 2000.into()),
        (99, U256::MAX),
    ] {
        s.insert(k.into(), v);
    }
    s.insert(mapping(44.into(), 0.into()), 123.into());
    s.insert(mapping(55.into(), mapping(44.into(), 1.into())), 9.into());
    s.insert(mapping(44.into(), 6.into()), 17.into());
    s
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
fn expected_role(pre: &State, r: U256, m: U256, route: Route, caller: U256) -> Expected {
    let mut x = Expected::new(pre);
    let authorized = match route {
        Route::Renounce => caller == m,
        _ => !get(pre, position(get(pre, admin(r)), caller)).is_zero(),
    };
    if !authorized {
        x.exit = Exit::Revert(error(match route {
            Route::Grant => "AccessControl: sender must be an admin to grant",
            Route::Revoke => "AccessControl: sender must be an admin to revoke",
            Route::Renounce => "AccessControl: can only renounce roles for self",
        }));
        return x;
    }
    let p = get(pre, position(r, m));
    let len = get(pre, head(r));
    if matches!(route, Route::Grant) && p.is_zero() {
        let next = len.overflowing_add(1.into()).0;
        x.store(head(r), next);
        x.store(element(r, len), m);
        x.store(position(r, m), next);
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
        let tail = get(pre, element(r, last));
        // This older library always self-swaps, including sole/tail removal.
        x.store(element(r, at), tail);
        x.store(position(r, tail), p);
        x.store(element(r, last), 0.into());
        x.store(head(r), last);
        x.store(position(r, m), 0.into());
        x.log(vec![role("RoleRevoked(bytes32,address,address)"), r, m, caller], vec![]);
    }
    x
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
        data: &[u8],
        caller: U256,
        account: U256,
        pre: &State,
        scope: &str,
        creation: bool,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<Execution> {
        let e = vm::execute_with_self_code_size(code, data, caller, account, pre, Some(if creation { 0 } else { 7896 }));
        self.calls += 1;
        let v = json!({"name":name,"signature":sig,"scope":scope,"code_kind":if creation {"bytecode"}else{"deployedBytecode"},"caller":w(caller),"address":w(account),"self_code_size":if creation {0}else{7896},"calldata":hex::encode(data),"prestate":state_json(pre),"execution":execution_json(&e)});
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
        self.record(name, sig, self.runtime, &call(sig, args), caller, address(ORI), pre, scope, false, save)
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
        let want = expected_role(pre, r, m, route, caller);
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
            let mut reads = vec![position(r, m)];
            if !matches!(route, Route::Renounce) {
                reads.extend([admin(r), position(get(pre, admin(r)), caller)]);
            }
            for key in reads {
                ensure!(e.reads.iter().any(|x| x.key == key), "{name}: authorization/position read omitted");
            }
            for input in [words(&[r, 8.into()]), words(&[m, head(r).overflowing_add(1.into()).0])] {
                ensure!(e.keccaks.iter().any(|h| h.input == input), "{name}: role/index preimage omitted");
            }
            if !e.writes.is_empty() {
                ensure!(e.keccaks.iter().any(|h| h.input == vm::word(head(r))), "{name}: array preimage omitted");
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
        let len = get(pre, head(r));
        ensure!(len < 16.into(), "bounded coherent readbacks");
        self.getter(name, "getRoleMemberCount(bytes32)", &[r], len, pre, save)?;
        self.getter(name, "getRoleAdmin(bytes32)", &[r], get(pre, admin(r)), pre, save)?;
        let mut members = BTreeSet::from([member, 0.into(), 99.into()]);
        for i in 0..len.as_usize() {
            let m = get(pre, element(r, i.into()));
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
                u8::from(!get(pre, position(r, m)).is_zero()).into(),
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
    pub fn constructor(&mut self, capture: &Value, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let who = capture["address"].as_str().context("capture address")?;
        let (label, name, symbol, fee) = if address(who) == address(ORI) {
            ("ori", "Orizon", "ORI", "0x7ce0b6cc0bd541ade69d836643869730b999971b")
        } else {
            ensure!(address(who) == address(FNA), "only two complete constructor bindings");
            ("fna", "FinTech AI", "FNA", "0x71407c1fa442793b8e836de862978d2a5d129bf5")
        };
        let args = bytes(&capture["creationBytecode"]["transformationValues"]["constructorArguments"])?;
        ensure!(args.len() == 288, "five-field constructor ABI size");
        let field = |i: usize| U256::from_big_endian(&args[i * 32..(i + 1) * 32]);
        ensure!(
            field(0) == address(fee) && field(1) == 100000.into() && field(2) == 20000.into(),
            "independent fee constructor values"
        );
        ensure!(field(3) == 160.into() && field(4) == 224.into(), "canonical dynamic offsets");
        for (offset, s) in [(160, name), (224, symbol)] {
            ensure!(U256::from_big_endian(&args[offset..offset + 32]) == s.len().into(), "ABI string length");
            ensure!(
                &args[offset + 32..offset + 32 + s.len()] == s.as_bytes() && args[offset + 32 + s.len()..offset + 64].iter().all(|b| *b == 0),
                "ABI string bytes/padding"
            );
        }
        let independent = [
            words(&[address(fee), 100000.into(), 20000.into(), 160.into(), 224.into()]),
            string_tail(name),
            string_tail(symbol),
        ]
        .concat();
        ensure!(args == independent, "independently reencoded full ABI append");
        let creation = bytes(&capture["creationBytecode"]["recompiledBytecode"])?;
        ensure!(creation.len() == 9347, "selected creation size");
        let code = [creation, args].concat();
        ensure!(code == bytes(&capture["creationBytecode"]["onchainBytecode"])?, "complete saved creation");
        let pre = State::new();
        let label = format!("{label}_constructor");
        let e = self.record(
            &label,
            "constructor(address,uint256,uint256,string,string)",
            &code,
            &[],
            address(capture["deployment"]["deployer"].as_str().context("deployer")?),
            address(who),
            &pre,
            "unsupported_path_control",
            true,
            save,
        )?;
        let mut want = Expected::new(&pre);
        want.store(3.into(), packed(name));
        want.store(4.into(), packed(symbol));
        want.store(5.into(), 9.into());
        want.prefix(&label, &e, &pre)?;
        unsupported(&label, &e, &pre, 0x46)?;
        ensure!(
            e.writes.iter().all(|s| [3.into(), 4.into(), 5.into()].contains(&s.key)) && e.logs.is_empty(),
            "constructor stops before domain and all role/fee writes"
        );
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
            let mut pre = base();
            let before: Vec<U256> = before.into_iter().map(U256::from).collect();
            seed(&mut pre, r, &before);
            let m = U256::from(member);
            let caller = if matches!(route, Route::Renounce) { m } else { 1.into() };
            let post = self.operation(name, r, m, route, caller, &pre, save)?;
            self.readbacks(name, r, m, &post, save)?;
        }
        for (i, r) in [U256::zero(), role("MINT"), role("INTERN_SYSTEM"), U256::MAX, (U256::one() << 255) + 7]
            .into_iter()
            .enumerate()
        {
            let pre = base();
            let name = format!("role_identity_{i}");
            let post = self.operation(&name, r, 3.into(), Route::Grant, 1.into(), &pre, save)?;
            self.readbacks(&name, r, 3.into(), &post, save)?;
        }
        let mut pre = base();
        let custom = role("CUSTOM_ADMIN");
        seed(&mut pre, custom, &[2.into()]);
        pre.insert(admin(r), custom);
        let post = self.operation("custom_admin_authorized", r, 3.into(), Route::Grant, 2.into(), &pre, save)?;
        self.readbacks("custom_admin_authorized", r, 3.into(), &post, save)?;
        for (name, route, caller) in [
            ("wrong_grant_admin", Route::Grant, 1),
            ("wrong_revoke_admin", Route::Revoke, 1),
            ("wrong_renounce_account", Route::Renounce, 2),
        ] {
            self.operation(name, r, 3.into(), route, caller.into(), &post, save)?;
        }
        let mut max_admin = base();
        seed(&mut max_admin, U256::MAX, &[2.into()]);
        max_admin.insert(admin(r), U256::MAX);
        let max_post = self.operation("max_admin_authorized", r, 3.into(), Route::Grant, 2.into(), &max_admin, save)?;
        self.readbacks("max_admin_authorized", r, 3.into(), &max_post, save)?;
        self.operation("max_admin_revoke", r, 3.into(), Route::Revoke, 2.into(), &max_post, save)?;
        let no_admin = self.operation("last_admin_renounce", 0.into(), 1.into(), Route::Renounce, 1.into(), &base(), save)?;
        self.readbacks("last_admin_renounce", 0.into(), 1.into(), &no_admin, save)?;
        self.operation("grant_after_last_admin_renounce", r, 3.into(), Route::Grant, 1.into(), &no_admin, save)?;
        self.operation("repeat_last_admin_renounce", 0.into(), 1.into(), Route::Renounce, 1.into(), &no_admin, save)?;
        let mut sequence = base();
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
        ensure!(normalized(&sequence) == normalized(&base()), "sequential restoration");
        self.malformed(save)?;
        self.raw_getters(save)?;
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
            let mut pre = base();
            pre.insert(head(r), len);
            pre.insert(position(r, m), pos);
            pre.insert(element(r, 0.into()), m);
            self.operation(name, r, m, Route::Revoke, 1.into(), &pre, save)?;
        }
        // Malformed successes are source controls, not coherent admission.
        let mut pre = base();
        pre.insert(head(r), U256::MAX);
        self.operation("malformed_max_length_push", r, m, Route::Grant, 1.into(), &pre, save)?;
        let mut pre = base();
        pre.insert(head(r), 1.into());
        pre.insert(element(r, 0.into()), 4.into());
        pre.insert(position(r, m), 1.into());
        pre.insert(position(r, 4.into()), 1.into());
        self.operation("malformed_index_points_to_other_member", r, m, Route::Revoke, 1.into(), &pre, save)?;
        let mut pre = base();
        seed(&mut pre, r, &[3.into(), 4.into()]);
        pre.insert(element(r, 1.into()), (U256::one() << 200) + 4);
        self.operation("malformed_dirty_tail_word", r, m, Route::Revoke, 1.into(), &pre, save)?;
        let mut pre = base();
        pre.insert(position(r, m), U256::MAX);
        self.operation("malformed_duplicate_ignores_length", r, m, Route::Grant, 1.into(), &pre, save)?;
        let ordinary = base();
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
                address(ORI),
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
            address(ORI),
            &ordinary,
            "synthetic_local_operation",
            false,
            save,
        )?;
        expected_role(&ordinary, r, 3.into(), Route::Grant, 1.into()).check("dirty_address_abi", &e, &ordinary)?;
        let extra = [call("grantRole(bytes32,address)", &[r, 3.into()]), vec![255; 32]].concat();
        let e = self.record(
            "trailing_role_abi",
            "grantRole(bytes32,address)",
            self.runtime,
            &extra,
            1.into(),
            address(ORI),
            &ordinary,
            "synthetic_local_operation",
            false,
            save,
        )?;
        expected_role(&ordinary, r, 3.into(), Route::Grant, 1.into()).check("trailing_role_abi", &e, &ordinary)?;
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
    fn raw_getters(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        for (i, value) in [U256::zero(), 1.into(), U256::one() << 255, U256::MAX].into_iter().enumerate() {
            let mut pre = base();
            pre.insert(mapping(44.into(), 0.into()), value);
            pre.insert(2.into(), value);
            pre.insert(mapping(55.into(), mapping(44.into(), 1.into())), value);
            pre.insert(mapping(44.into(), 6.into()), value);
            for (sig, args, key) in [
                ("balanceOf(address)", vec![44.into()], mapping(44.into(), 0.into())),
                ("totalSupply()", vec![], 2.into()),
                (
                    "allowance(address,address)",
                    vec![44.into(), 55.into()],
                    mapping(55.into(), mapping(44.into(), 1.into())),
                ),
                ("nonces(address)", vec![44.into()], mapping(44.into(), 6.into())),
            ] {
                let name = format!("raw_getter_{i}");
                self.getter(&name, sig, &args, get(&pre, key), &pre, save)?;
                let last = self.cases.last().unwrap();
                ensure!(
                    last["execution"]["reads"].as_array().unwrap().len() == 1 && last["execution"]["reads"][0]["key"] == w(key),
                    "{sig}: exact single raw read"
                );
            }
        }
        // PHI gains only exact-runtime attribution, never invented creation data.
        for account in [ORI, FNA, PHI] {
            let pre = base();
            let sig = "balanceOf(address)";
            let e = self.record(
                "runtime_identity_raw_getter",
                sig,
                self.runtime,
                &call(sig, &[44.into()]),
                9.into(),
                address(account),
                &pre,
                "synthetic_local_operation",
                false,
                save,
            )?;
            let mut x = Expected::new(&pre);
            x.exit = Exit::Return(vm::word(123.into()).to_vec());
            x.check("runtime_identity_raw_getter", &e, &pre)?;
        }
        Ok(())
    }
    fn excluded(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let mut pre = base();
        pre.insert(mapping(3.into(), 0.into()), 100.into());
        pre.insert(2.into(), 223.into()); // Exact sum of the two synthetic balances.
        let e = self.run(
            "excluded_fee_receiver",
            "transfer(address,uint256)",
            &[77.into(), 100.into()],
            3.into(),
            &pre,
            "unsupported_path_control",
            save,
        )?;
        let mut x = Expected::new(&pre);
        x.store(mapping(3.into(), 0.into()), 0.into());
        x.store(mapping(78.into(), 0.into()), 1.into());
        x.log(
            vec![role("Transfer(address,address,uint256)"), 3.into(), 78.into()],
            vm::word(1.into()).to_vec(),
        );
        x.log(
            vec![role("FeeTaken(address,address,bool,uint256,uint256)"), 3.into(), 78.into()],
            words(&[0.into(), 99.into(), 1.into()]),
        );
        x.prefix("excluded_fee_receiver", &e, &pre)?;
        unsupported("excluded_fee_receiver", &e, &pre, 0x3b)?;
        ensure!(
            matches!(&e.exit,Exit::HarnessFailure(s) if s=="EXTCODESIZE unknown external account"),
            "fee receiver must not be mocked"
        );
        let e = self.run(
            "excluded_permit",
            "permit(address,address,uint256,uint256,uint8,bytes32,bytes32)",
            &[3.into(), 4.into(), 1.into(), U256::MAX, 27.into(), 1.into(), 1.into()],
            3.into(),
            &pre,
            "unsupported_path_control",
            save,
        )?;
        Expected::new(&pre).prefix("excluded_permit", &e, &pre)?;
        unsupported("excluded_permit", &e, &pre, 0x42)?;
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
