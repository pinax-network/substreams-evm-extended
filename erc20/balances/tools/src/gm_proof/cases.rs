//! Capture-bound synthetic local GM operations, not proxy dispatch or admission.
//! Expectations follow the captured source; actual compiled traces are a separate gate.
use super::{address, vm, IMPLEMENTATION, PROXY, PROXY_B};
use crate::ptoken_proof::cases::{call, execution_json, get, mapping, role, state_json, w};
use anyhow::{ensure, Result};
use primitive_types::U256;
use serde_json::{json, Value};
use vm::{Execution, Exit, State};

pub fn member_key(r: U256, m: U256) -> U256 {
    mapping(m, mapping(r, 201.into()))
}
pub fn role_admin(r: U256) -> U256 {
    mapping(r, 201.into()).overflowing_add(1.into()).0
}
pub fn head(r: U256) -> U256 {
    mapping(r, 251.into())
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
    let mut out = [0; 32];
    out[..s.len()].copy_from_slice(s.as_bytes());
    out[31] = (s.len() * 2) as u8;
    U256::from_big_endian(&out)
}
fn words(v: &[U256]) -> Vec<u8> {
    v.iter().flat_map(|v| vm::word(*v)).collect()
}
fn string_abi(s: &str) -> Vec<u8> {
    let mut out = words(&[32.into(), s.len().into()]);
    out.extend(s.as_bytes());
    out.resize(out.len().div_ceil(32) * 32, 0);
    out
}
fn error(s: &str) -> Vec<u8> {
    [call("Error(string)", &[]), string_abi(s)].concat()
}
fn access_error(caller: U256, admin: U256) -> Vec<u8> {
    error(&format!(
        "AccessControl: account 0x{} is missing role {}",
        hex::encode(&vm::word(caller)[12..]),
        w(admin)
    ))
}
pub fn initializer_arguments(name: &str, symbol: &str, compliance: U256, manager: U256) -> Vec<u8> {
    let a = string_abi(name);
    let b = string_abi(symbol);
    let mut out = call(
        "initialize(string,string,address,address)",
        &[128.into(), (128 + a.len() - 32).into(), compliance, manager],
    );
    out.extend(&a[32..]);
    out.extend(&b[32..]);
    out
}
fn seed_role(s: &mut State, r: U256, members: &[U256]) {
    // Only small coherent states are passed here; pathological lengths are seeded separately.
    for i in 0..get(s, head(r)).as_usize() {
        let m = get(s, element(r, i.into()));
        s.remove(&element(r, i.into()));
        s.remove(&position(r, m));
        s.remove(&member_key(r, m));
    }
    s.insert(head(r), members.len().into());
    for (i, m) in members.iter().enumerate() {
        s.insert(member_key(r, *m), 1.into());
        s.insert(element(r, i.into()), *m);
        s.insert(position(r, *m), (i + 1).into());
    }
}
fn base() -> State {
    let mut s = State::new();
    s.insert(0.into(), 1.into());
    seed_role(&mut s, 0.into(), &[1.into()]);
    seed_role(&mut s, role("UNRELATED_ROLE"), &[88.into()]);
    s.insert(mapping(44.into(), 51.into()), 123.into());
    s.insert(53.into(), 123.into());
    s.insert(mapping(55.into(), mapping(44.into(), 52.into())), 9.into());
    for (k, v) in [
        (54, packed("Base")),
        (55, packed("B")),
        (301, 77.into()),
        (351, 78.into()),
        (401, packed("Override")),
        (402, packed("OV")),
    ] {
        s.insert(k.into(), v);
    }
    // Unrelated reserve/gap and beacon-pointer sentinels must survive every local role operation.
    s.insert(99.into(), U256::MAX);
    s.insert(
        U256::from_str_radix("a3f0ad74e5423aebfd80d3ef4346578335a9a72aeaee59ff6cb3582b35133d50", 16).unwrap(),
        79.into(),
    );
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
    fn store(&mut self, key: U256, new: U256) {
        self.effects.push(Effect::Store(key, get(&self.state, key), new));
        self.state.insert(key, new);
    }
    fn log(&mut self, topics: Vec<U256>, data: Vec<u8>) {
        self.effects.push(Effect::Log(topics, data));
    }
    fn revert(&mut self, data: Vec<u8>) {
        self.exit = Exit::Revert(data);
    }
    fn check(&self, name: &str, e: &Execution, pre: &State) -> Result<()> {
        ensure!(e.exit == self.exit, "{name}: exit {:?}, expected {:?}", e.exit, self.exit);
        self.check_prefix(name, e)?;
        if matches!(self.exit, Exit::Return(_)) {
            ensure!(normalized(&e.committed) == normalized(&self.state), "{name}: all-cell committed state");
            ensure!(e.committed_logs == e.logs, "{name}: all logs committed");
        } else {
            rollback(name, e, pre)?;
        }
        Ok(())
    }
    fn check_prefix(&self, name: &str, e: &Execution) -> Result<()> {
        let mut effects: Vec<_> = e
            .writes
            .iter()
            .map(|s| (s.step, Effect::Store(s.key, s.old, s.new)))
            .chain(e.logs.iter().map(|l| (l.step, Effect::Log(l.topics.clone(), l.data.clone()))))
            .collect();
        effects.sort_by_key(|(step, _)| *step);
        ensure!(
            effects.into_iter().map(|(_, e)| e).collect::<Vec<_>>() == self.effects,
            "{name}: exact ordered attempted stores/logs differ; expected {:?}; stores {:?}; logs {:?}",
            self.effects,
            e.writes,
            e.logs
        );
        for h in &e.keccaks {
            ensure!(vm::hash(&h.input) == h.output, "{name}: hash witness");
        }
        Ok(())
    }
}
fn rollback(name: &str, e: &Execution, pre: &State) -> Result<()> {
    ensure!(e.committed == *pre && e.committed_logs.is_empty(), "{name}: complete rollback");
    Ok(())
}
fn expected_role(pre: &State, r: U256, m: U256, grant: bool, caller: U256) -> Expected {
    let mut x = Expected::new(pre);
    let key = member_key(r, m);
    let old = get(pre, key);
    if (old & U256::from(255)).is_zero() == grant {
        x.store(key, (old & !U256::from(255)) | U256::from(u8::from(grant)));
        x.log(
            vec![
                role(if grant {
                    "RoleGranted(bytes32,address,address)"
                } else {
                    "RoleRevoked(bytes32,address,address)"
                }),
                r,
                m,
                caller,
            ],
            vec![],
        );
    }
    // The captured older void-super override always calls the set operation.
    let index = get(pre, position(r, m));
    let len = get(pre, head(r));
    if grant && index.is_zero() {
        let next = len.overflowing_add(1.into()).0;
        x.store(head(r), next);
        x.store(element(r, len), m);
        x.store(position(r, m), next);
    } else if !grant && !index.is_zero() {
        if len.is_zero() {
            x.revert(call("Panic(uint256)", &[0x11.into()]));
            return x;
        }
        let tail_index = len - 1;
        let index = index - 1;
        if tail_index != index {
            if index >= len {
                x.revert(call("Panic(uint256)", &[0x32.into()]));
                return x;
            }
            let tail = get(pre, element(r, tail_index));
            x.store(element(r, index), tail);
            x.store(position(r, tail), index + 1);
        }
        x.store(element(r, tail_index), 0.into());
        x.store(head(r), tail_index);
        x.store(position(r, m), 0.into());
    }
    x
}
#[derive(Clone, Copy)]
enum Route {
    Grant,
    Revoke,
    Renounce,
}
impl Route {
    fn sig(self) -> &'static str {
        match self {
            Self::Grant => "grantRole(bytes32,address)",
            Self::Revoke => "revokeRole(bytes32,address)",
            Self::Renounce => "renounceRole(bytes32,address)",
        }
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
        data: &[u8],
        caller: U256,
        account: U256,
        size: Option<usize>,
        pre: &State,
        unsupported: bool,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<Execution> {
        let e = vm::execute_with_self_code_size(code, data, caller, account, pre, size);
        self.calls += 1;
        let record = json!({"name":name,"signature":sig,"scope":if unsupported {"unsupported_path_control"}else{"synthetic_local_operation"},"caller":w(caller),"address":w(account),"self_code_size":size,"calldata":hex::encode(data),"prestate":state_json(pre),"execution":execution_json(&e)});
        save(self.calls, &record)?;
        self.cases.push(record);
        if !unsupported {
            ensure!(!matches!(e.exit, Exit::Invalid | Exit::HarnessFailure(_)), "{name}: unexpected {:?}", e.exit);
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
        self.record(name, sig, self.runtime, &call(sig, args), caller, address(PROXY), Some(824), pre, false, save)
    }
    pub fn constructor(&mut self, creation: &[u8], save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let pre = State::new();
        let e = self.record(
            "implementation_constructor",
            "constructor()",
            creation,
            &[],
            9.into(),
            address(IMPLEMENTATION),
            Some(0),
            &pre,
            false,
            save,
        )?;
        let mut want = Expected::new(&pre);
        want.store(0.into(), 255.into());
        want.log(vec![role("Initialized(uint8)")], vm::word(255.into()).to_vec());
        want.exit = Exit::Return(self.runtime.to_vec());
        want.check("implementation_constructor", &e, &pre)
    }
    fn getter(&mut self, name: &str, sig: &str, args: &[U256], want: Vec<u8>, pre: &State, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let e = self.run(name, sig, args, 9.into(), pre, save)?;
        let mut x = Expected::new(pre);
        x.exit = Exit::Return(want);
        x.check(name, &e, pre)?;
        ensure!(e.committed == *pre, "{name}: getter unchanged including explicit zero cells");
        Ok(())
    }
    fn readbacks(&mut self, name: &str, r: U256, checked: U256, pre: &State, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let len = get(pre, head(r));
        ensure!(len < 16.into(), "bounded readback state");
        self.getter(name, "getRoleMemberCount(bytes32)", &[r], vm::word(len).to_vec(), pre, save)?;
        self.getter(name, "getRoleAdmin(bytes32)", &[r], vm::word(get(pre, role_admin(r))).to_vec(), pre, save)?;
        let mut checked_members = std::collections::BTreeSet::from([checked, 0.into(), 99.into()]);
        for i in 0..len.as_usize() {
            let m = get(pre, element(r, i.into()));
            checked_members.insert(m);
            self.getter(
                name,
                "getRoleMember(bytes32,uint256)",
                &[r, i.into()],
                vm::word(m & ((U256::one() << 160) - 1)).to_vec(),
                pre,
                save,
            )?;
        }
        for m in checked_members {
            self.getter(
                name,
                "hasRole(bytes32,address)",
                &[r, m],
                vm::word(U256::from(u8::from(!(get(pre, member_key(r, m)) & U256::from(255)).is_zero()))).to_vec(),
                pre,
                save,
            )?;
        }
        for index in [len, U256::MAX] {
            let e = self.run(name, "getRoleMember(bytes32,uint256)", &[r, index], 9.into(), pre, save)?;
            let mut x = Expected::new(pre);
            x.revert(call("Panic(uint256)", &[0x32.into()]));
            x.check(name, &e, pre)?;
        }
        for (sig, args, want) in [
            ("balanceOf(address)", vec![44.into()], get(pre, mapping(44.into(), 51.into()))),
            ("totalSupply()", vec![], get(pre, 53.into())),
            (
                "allowance(address,address)",
                vec![44.into(), 55.into()],
                get(pre, mapping(55.into(), mapping(44.into(), 52.into()))),
            ),
        ] {
            self.getter(name, sig, &args, vm::word(want).to_vec(), pre, save)?;
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn operation(
        &mut self,
        name: &str,
        r: U256,
        before: &[U256],
        m: U256,
        route: Route,
        previous: Option<State>,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<State> {
        let mut pre = previous.unwrap_or_else(base);
        seed_role(&mut pre, r, before);
        let caller = if matches!(route, Route::Renounce) { m } else { 1.into() };
        let e = self.run(name, route.sig(), &[r, m], caller, &pre, save)?;
        let want = expected_role(&pre, r, m, matches!(route, Route::Grant), caller);
        want.check(name, &e, &pre)?;
        for key in [member_key(r, m), position(r, m)] {
            ensure!(
                e.reads.iter().any(|v| v.key == key && v.value == get(&pre, key)),
                "{name}: boolean/index witness"
            );
        }
        if !matches!(route, Route::Renounce) {
            for key in [role_admin(r), member_key(get(&pre, role_admin(r)), caller)] {
                ensure!(e.reads.iter().any(|v| v.key == key && v.value == get(&pre, key)), "{name}: admin read witness");
            }
        }
        for input in [
            words(&[r, 201.into()]),
            words(&[m, mapping(r, 201.into())]),
            words(&[r, 251.into()]),
            words(&[m, head(r) + 1]),
        ] {
            ensure!(e.keccaks.iter().any(|h| h.input == input), "{name}: linked role/set hash preimage");
        }
        self.readbacks(name, r, m, &e.committed, save)?;
        Ok(e.committed)
    }
    pub fn matrix(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let r = role("MATRIX_ROLE");
        for (name, before, m, grant) in [
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
            let before = before.into_iter().map(|x| U256::from(x as u64)).collect::<Vec<_>>();
            self.operation(
                &format!("role_{name}"),
                r,
                &before,
                U256::from(m as u64),
                if grant { Route::Grant } else { Route::Revoke },
                None,
                save,
            )?;
        }
        for (name, r, before) in [
            ("default_admin", 0.into(), vec![1.into()]),
            ("minter", role("MINTER_ROLE"), vec![]),
            ("burner", role("BURNER_ROLE"), vec![]),
            ("configurer", role("CONFIGURER_ROLE"), vec![]),
            ("arbitrary", U256::one() << 255, vec![]),
            ("max_role", U256::MAX, vec![]),
        ] {
            self.operation(name, r, &before, (U256::one() << 160) - 1, Route::Grant, None, save)?;
        }
        for (name, r, before, m) in [
            ("renounce_present", r, vec![3.into(), 4.into()], 3.into()),
            ("renounce_absent", r, vec![4.into()], 3.into()),
            ("renounce_zero", r, vec![0.into()], 0.into()),
            ("renounce_last_admin", 0.into(), vec![1.into()], 1.into()),
        ] {
            self.operation(name, r, &before, m, Route::Renounce, None, save)?;
        }
        self.sequences(save)?;
        self.incoherent(save)?;
        self.permissions(save)?;
        self.initializers(save)?;
        self.raw_getters(save)?;
        self.boundaries(save)?;
        Ok(())
    }
    fn sequences(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let r = role("SEQUENCE_ROLE");
        let mut s = base();
        let mut members = vec![];
        for (i, (m, route)) in [
            (3, Route::Grant),
            (0, Route::Grant),
            (4, Route::Grant),
            (3, Route::Grant),
            (3, Route::Revoke),
            (4, Route::Renounce),
            (3, Route::Grant),
            (0, Route::Revoke),
            (3, Route::Revoke),
            (3, Route::Revoke),
        ]
        .into_iter()
        .enumerate()
        {
            let m = U256::from(m as u64);
            let name = format!("sequence_{i}");
            // Use actual committed state, never reseed it between sequence calls.
            let pre = s;
            let caller = if matches!(route, Route::Renounce) { m } else { 1.into() };
            let e = self.run(&name, route.sig(), &[r, m], caller, &pre, save)?;
            expected_role(&pre, r, m, matches!(route, Route::Grant), caller).check(&name, &e, &pre)?;
            if matches!(route, Route::Grant) {
                if !members.contains(&m) {
                    members.push(m);
                }
            } else if let Some(index) = members.iter().position(|v| *v == m) {
                members.swap_remove(index);
            }
            ensure!(get(&e.committed, head(r)) == members.len().into(), "sequence length");
            for (j, m) in members.iter().enumerate() {
                ensure!(
                    get(&e.committed, element(r, j.into())) == *m && get(&e.committed, position(r, *m)) == (j + 1).into(),
                    "sequence members"
                );
            }
            s = e.committed;
            self.readbacks(&name, r, m, &s, save)?;
        }
        Ok(())
    }
    fn incoherent(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let r = role("INCOHERENT_ROLE");
        let m = U256::from(3);
        for (name, b, indexed, grant) in [
            ("incoherent_set_only_grant", true, false, true),
            ("incoherent_bool_only_grant", false, true, true),
            ("incoherent_bool_only_revoke", true, false, false),
            ("incoherent_set_only_revoke", false, true, false),
        ] {
            let mut pre = base();
            let members = if indexed { vec![m, 4.into()] } else { vec![] };
            seed_role(&mut pre, r, &members);
            pre.insert(member_key(r, m), u8::from(b).into());
            let sig = if grant { "grantRole(bytes32,address)" } else { "revokeRole(bytes32,address)" };
            let e = self.run(name, sig, &[r, m], 1.into(), &pre, save)?;
            expected_role(&pre, r, m, grant, 1.into()).check(name, &e, &pre)?;
            self.readbacks(name, r, m, &e.committed, save)?;
        }
        for (name, len, index) in [
            ("empty_set_prefix_revert", 0u64, 1.into()),
            ("index_out_of_bounds_prefix_revert", 1, 3.into()),
            ("max_index_prefix_revert", 1, U256::MAX),
        ] {
            let mut pre = base();
            pre.insert(member_key(r, m), 1.into());
            pre.insert(head(r), len.into());
            pre.insert(position(r, m), index);
            pre.insert(element(r, 0.into()), 4.into());
            let e = self.run(name, "revokeRole(bytes32,address)", &[r, m], 1.into(), &pre, save)?;
            expected_role(&pre, r, m, false, 1.into()).check(name, &e, &pre)?;
        }
        for (name, members, index_tail) in [
            ("incorrect_array_member", vec![5.into(), 4.into()], 2u64),
            ("duplicate_array_entries", vec![3.into(), 3.into()], 2),
            ("moved_tail_wrong_index", vec![3.into(), 4.into()], 99),
        ] {
            let mut pre = base();
            seed_role(&mut pre, r, &members);
            pre.insert(member_key(r, m), 1.into());
            pre.insert(position(r, m), 1.into());
            let tail = *members.last().unwrap();
            pre.insert(position(r, tail), index_tail.into());
            if name == "duplicate_array_entries" {
                pre.insert(position(r, m), 1.into());
            }
            let e = self.run(name, "revokeRole(bytes32,address)", &[r, m], 1.into(), &pre, save)?;
            expected_role(&pre, r, m, false, 1.into()).check(name, &e, &pre)?;
            self.readbacks(name, r, m, &e.committed, save)?;
        }
        for (name, old, grant) in [
            ("dirty_bool_two", 2.into(), false),
            ("dirty_bool_high_bits", (U256::one() << 200) | U256::from(2), false),
            ("dirty_bool_zero_byte_grant", U256::one() << 200, true),
            ("dirty_bool_duplicate", 2.into(), true),
        ] {
            let mut pre = base();
            let single_member = [m];
            seed_role(&mut pre, r, if grant && name != "dirty_bool_duplicate" { &[] } else { &single_member });
            pre.insert(member_key(r, m), old);
            let sig = if grant { "grantRole(bytes32,address)" } else { "revokeRole(bytes32,address)" };
            let e = self.run(name, sig, &[r, m], 1.into(), &pre, save)?;
            expected_role(&pre, r, m, grant, 1.into()).check(name, &e, &pre)?;
            self.readbacks(name, r, m, &e.committed, save)?;
        }
        let mut pre = base();
        pre.insert(head(r), U256::MAX);
        let e = self.run("incoherent_max_length_push", "grantRole(bytes32,address)", &[r, m], 1.into(), &pre, save)?;
        expected_role(&pre, r, m, true, 1.into()).check("incoherent_max_length_push", &e, &pre)?;
        Ok(())
    }
    fn permissions(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
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
                "renounce_wrong_caller",
                "renounceRole(bytes32,address)",
                vec![r, 3.into()],
                9,
                error("AccessControl: can only renounce roles for self"),
            ),
            (
                "admin_cannot_mint",
                "mint(address,uint256)",
                vec![44.into(), 1.into()],
                1,
                access_error(1.into(), role("MINTER_ROLE")),
            ),
            (
                "admin_cannot_burn",
                "burn(address,uint256)",
                vec![44.into(), 1.into()],
                1,
                access_error(1.into(), role("BURNER_ROLE")),
            ),
            (
                "admin_cannot_set_compliance",
                "setCompliance(address)",
                vec![80.into()],
                1,
                access_error(1.into(), role("CONFIGURER_ROLE")),
            ),
            (
                "admin_cannot_set_manager",
                "setTokenPauseManager(address)",
                vec![80.into()],
                1,
                access_error(1.into(), role("CONFIGURER_ROLE")),
            ),
        ] {
            let pre = base();
            let e = self.run(name, sig, &args, U256::from(caller as u64), &pre, save)?;
            let mut want = Expected::new(&pre);
            want.revert(expected);
            want.check(name, &e, &pre)?;
        }
        for admin in [role("CUSTOM_ADMIN"), U256::one() << 255, U256::MAX] {
            let mut pre = base();
            pre.insert(role_admin(r), admin);
            seed_role(&mut pre, admin, &[2.into()]);
            for (name, caller) in [("custom_admin_wrong", 1u64), ("custom_admin_correct", 2)] {
                let e = self.run(name, "grantRole(bytes32,address)", &[r, 3.into()], caller.into(), &pre, save)?;
                let want = if caller == 2 {
                    expected_role(&pre, r, 3.into(), true, caller.into())
                } else {
                    let mut x = Expected::new(&pre);
                    x.revert(access_error(caller.into(), admin));
                    x
                };
                want.check(name, &e, &pre)?;
                ensure!(
                    e.reads.iter().any(|v| v.key == role_admin(r) && v.value == admin),
                    "arbitrary admin linkage read"
                );
            }
            self.getter("custom_admin_getter", "getRoleAdmin(bytes32)", &[r], vm::word(admin).to_vec(), &pre, save)?;
        }
        let mut dynamic = initializer_arguments("GM", "G", 77.into(), 78.into());
        dynamic[4..36].copy_from_slice(&vm::word(U256::MAX));
        let mut short_dynamic = initializer_arguments("GM", "G", 77.into(), 78.into());
        short_dynamic.truncate(4 + 128 + 31);
        let mut bad_length = initializer_arguments("GM", "G", 77.into(), 78.into());
        bad_length[132..164].copy_from_slice(&vm::word(U256::MAX));
        for (name, data) in [
            ("dirty_role_address", call("grantRole(bytes32,address)", &[r, U256::one() << 160])),
            ("dirty_getter_address", call("balanceOf(address)", &[U256::one() << 160])),
            ("short_role_calldata", call("grantRole(bytes32,address)", &[r])),
            ("short_selector", vec![0xff; 3]),
            ("unknown_selector", vec![0xff; 4]),
            ("no_public_full_role_array", call("getRoleMembers(bytes32)", &[r])),
            ("initializer_bad_offset", dynamic),
            ("initializer_short_dynamic", short_dynamic),
            ("initializer_max_dynamic_length", bad_length),
            ("initializer_dirty_pointer", initializer_arguments("GM", "G", U256::one() << 160, 78.into())),
        ] {
            let pre = State::new();
            let e = self.record(
                name,
                "malformed ABI",
                self.runtime,
                &data,
                1.into(),
                address(PROXY),
                Some(824),
                &pre,
                false,
                save,
            )?;
            let mut want = Expected::new(&pre);
            // solc 0.8.16 generated string decoder checks length > uint64::MAX
            // with Panic(0x41) before checking the available calldata tail.
            want.revert(if name == "initializer_max_dynamic_length" {
                call("Panic(uint256)", &[0x41.into()])
            } else {
                vec![]
            });
            want.check(name, &e, &pre)?;
        }
        // These setters are entirely local; do not confuse them with calls to the clients.
        for (name, sig, slot, event, zero_error, log_first) in [
            (
                "compliance",
                "setCompliance(address)",
                301,
                "ComplianceSet(address,address)",
                "ComplianceZeroAddress()",
                false,
            ),
            (
                "manager",
                "setTokenPauseManager(address)",
                351,
                "TokenPauseManagerSet(address,address)",
                "TokenPauseManagerCantBeZero()",
                true,
            ),
        ] {
            for new in [U256::zero(), 80.into(), U256::from(if slot == 301 { 77u64 } else { 78 })] {
                let mut pre = base();
                seed_role(&mut pre, role("CONFIGURER_ROLE"), &[2.into()]);
                let n = format!("local_set_{name}_{}", w(new));
                let e = self.run(&n, sig, &[new], 2.into(), &pre, save)?;
                let mut x = Expected::new(&pre);
                if new.is_zero() {
                    x.revert(call(zero_error, &[]));
                } else {
                    let topics = vec![role(event), get(&pre, slot.into()), new];
                    if log_first {
                        x.log(topics.clone(), vec![]);
                    }
                    x.store(slot.into(), new);
                    if !log_first {
                        x.log(topics, vec![]);
                    }
                }
                x.check(&n, &e, &pre)?;
            }
        }
        Ok(())
    }
    fn initializers(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let sig = "initialize(string,string,address,address)";
        for (name, n, s, compliance, manager, account, size, start_flag) in [
            ("initialize_short", "GM Token", "GM", 77u64, 78u64, PROXY, 824usize, 0u64),
            ("initialize_empty", "", "", 77, 78, PROXY, 824, 0),
            ("initialize_proxy_b", "GM B", "GMB", 81, 82, PROXY_B, 824, 0),
            ("initialize_zero_compliance", "GM", "G", 0, 78, PROXY, 824, 0),
            ("initialize_zero_manager", "GM", "G", 77, 0, PROXY, 824, 0),
            ("initialize_constructor_context", "GM", "G", 77, 78, IMPLEMENTATION, 0, 1),
        ] {
            let mut pre = State::new();
            if start_flag != 0 {
                pre.insert(0.into(), start_flag.into());
            }
            let data = initializer_arguments(n, s, compliance.into(), manager.into());
            let e = self.record(name, sig, self.runtime, &data, 9.into(), address(account), Some(size), &pre, false, save)?;
            let mut want = Expected::new(&pre);
            want.store(0.into(), 1.into());
            want.store(0.into(), 257.into());
            want.store(54.into(), packed(n));
            want.store(55.into(), packed(s));
            if compliance == 0 {
                want.revert(call("ComplianceZeroAddress()", &[]));
            } else {
                want.store(301.into(), compliance.into());
                want.log(vec![role("ComplianceSet(address,address)"), 0.into(), compliance.into()], vec![]);
                if manager == 0 {
                    want.revert(call("TokenPauseManagerCantBeZero()", &[]));
                } else {
                    want.log(vec![role("TokenPauseManagerSet(address,address)"), 0.into(), manager.into()], vec![]);
                    want.store(351.into(), manager.into());
                    let role_effects = expected_role(&want.state, 0.into(), 9.into(), true, 9.into());
                    want.effects.extend(role_effects.effects);
                    want.state = role_effects.state;
                    want.store(401.into(), packed(n));
                    want.store(402.into(), packed(s));
                    want.store(0.into(), 1.into());
                    want.log(vec![role("Initialized(uint8)")], vm::word(1.into()).to_vec());
                }
            }
            want.check(name, &e, &pre)?;
            if matches!(e.exit, Exit::Return(_)) {
                self.getter(name, "name()", &[], string_abi(n), &e.committed, save)?;
                self.getter(name, "symbol()", &[], string_abi(s), &e.committed, save)?;
                self.getter(name, "compliance()", &[], vm::word(compliance.into()).to_vec(), &e.committed, save)?;
                self.getter(name, "tokenPauseManager()", &[], vm::word(manager.into()).to_vec(), &e.committed, save)?;
                self.readbacks(name, 0.into(), 9.into(), &e.committed, save)?;
                let repeat = self.record(
                    &format!("{name}_repeat_deployed"),
                    sig,
                    self.runtime,
                    &data,
                    9.into(),
                    address(account),
                    Some(if account == IMPLEMENTATION { 8271 } else { 824 }),
                    &e.committed,
                    false,
                    save,
                )?;
                let mut x = Expected::new(&e.committed);
                x.revert(error("Initializable: contract is already initialized"));
                x.check(name, &repeat, &e.committed)?;
            }
        }
        for (name, flag, account, size) in [
            ("locked_implementation_initializer", 255u64, IMPLEMENTATION, 8271),
            ("nested_deployed_initializer", 257, PROXY, 824),
        ] {
            let pre = State::from([(0.into(), flag.into())]);
            let e = self.record(
                name,
                sig,
                self.runtime,
                &initializer_arguments("GM", "G", 77.into(), 78.into()),
                9.into(),
                address(account),
                Some(size),
                &pre,
                false,
                save,
            )?;
            let mut want = Expected::new(&pre);
            want.revert(error("Initializable: contract is already initialized"));
            want.check(name, &e, &pre)?;
        }
        Ok(())
    }
    fn raw_getters(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        for value in [U256::zero(), 1.into(), U256::MAX] {
            let mut pre = base();
            let second = if value > 1.into() { 1.into() } else { 0.into() };
            pre.insert(mapping(44.into(), 51.into()), value - second);
            pre.insert(mapping(45.into(), 51.into()), second);
            pre.insert(53.into(), value);
            pre.insert(mapping(55.into(), mapping(44.into(), 52.into())), value);
            pre.insert(301.into(), U256::zero());
            pre.insert(351.into(), (U256::one() << 160) - 1);
            seed_role(&mut pre, role("MINTER_ROLE"), &[45.into()]);
            pre.insert(401.into(), packed("Independent"));
            pre.insert(402.into(), packed("I"));
            for account in [PROXY, PROXY_B, IMPLEMENTATION] {
                for (sig, args, key) in [
                    ("balanceOf(address)", vec![44.into()], mapping(44.into(), 51.into())),
                    ("balanceOf(address)", vec![45.into()], mapping(45.into(), 51.into())),
                    ("balanceOf(address)", vec![0.into()], mapping(0.into(), 51.into())),
                    ("totalSupply()", vec![], 53.into()),
                    (
                        "allowance(address,address)",
                        vec![44.into(), 55.into()],
                        mapping(55.into(), mapping(44.into(), 52.into())),
                    ),
                ] {
                    let name = format!("raw_getter_{}_{}", w(value), account);
                    let e = self.record(
                        &name,
                        sig,
                        self.runtime,
                        &call(sig, &args),
                        9.into(),
                        address(account),
                        Some(if account == IMPLEMENTATION { 8271 } else { 824 }),
                        &pre,
                        false,
                        save,
                    )?;
                    let mut want = Expected::new(&pre);
                    want.exit = Exit::Return(vm::word(get(&pre, key)).to_vec());
                    want.check(&name, &e, &pre)?;
                    ensure!(
                        e.reads.len() == 1 && e.reads[0].key == key && e.reads[0].value == get(&pre, key),
                        "{name}: exact single raw storage read"
                    );
                }
            }
            self.getter("override_not_base_name", "name()", &[], string_abi("Independent"), &pre, save)?;
            self.getter("override_not_base_symbol", "symbol()", &[], string_abi("I"), &pre, save)?;
        }
        for (sig, value) in [
            ("DEFAULT_ADMIN_ROLE()", U256::zero()),
            ("MINTER_ROLE()", role("MINTER_ROLE")),
            ("BURNER_ROLE()", role("BURNER_ROLE")),
            ("CONFIGURER_ROLE()", role("CONFIGURER_ROLE")),
            ("decimals()", 18.into()),
        ] {
            self.getter("public_constant", sig, &[], vm::word(value).to_vec(), &base(), save)?;
        }
        Ok(())
    }
    fn boundaries(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let pre = State::from([(0.into(), 1.into())]);
        let e = self.record(
            "missing_self_context",
            "initialize(string,string,address,address)",
            self.runtime,
            &initializer_arguments("GM", "G", 77.into(), 78.into()),
            9.into(),
            address(PROXY),
            None,
            &pre,
            true,
            save,
        )?;
        ensure!(
            e.exit == Exit::HarnessFailure("EXTCODESIZE self context not provided".into()),
            "missing context must be harness failure: {:?}",
            e.exit
        );
        Expected::new(&pre).check_prefix("missing_self_context", &e)?;
        rollback("missing_self_context", &e, &pre)?;
        ensure!(e.trace.last().is_some_and(|s| s.pc == 1889 && s.opcode == 0x3b), "exact self context boundary");
        for (name, sig, args, caller, allowance_prefix) in [
            ("excluded_transfer", "transfer(address,uint256)", vec![56.into(), 1.into()], 44u64, false),
            ("excluded_mint", "mint(address,uint256)", vec![56.into(), 1.into()], 2, false),
            ("excluded_admin_burn", "burn(address,uint256)", vec![44.into(), 1.into()], 2, false),
            ("excluded_self_burn", "burn(uint256)", vec![1.into()], 44, false),
            (
                "excluded_transfer_from",
                "transferFrom(address,address,uint256)",
                vec![44.into(), 56.into(), 1.into()],
                55,
                true,
            ),
            ("excluded_burn_from", "burnFrom(address,uint256)", vec![44.into(), 1.into()], 55, true),
        ] {
            let mut pre = base();
            seed_role(&mut pre, role("MINTER_ROLE"), &[2.into()]);
            seed_role(&mut pre, role("BURNER_ROLE"), &[2.into()]);
            let e = self.record(
                name,
                sig,
                self.runtime,
                &call(sig, &args),
                caller.into(),
                address(PROXY),
                Some(824),
                &pre,
                true,
                save,
            )?;
            // The sole runtime GAS at 5556 is immediately before pause-manager STATICCALL.
            // No external call or compliance behavior executes in this bounded VM.
            ensure!(
                e.exit == Exit::HarnessFailure("unsupported opcode 0x5a at pc 5556".into()),
                "{name}: exact pre-call GAS boundary {:?}",
                e.exit
            );
            ensure!(
                e.trace.last().is_some_and(|s| s.pc == 5556 && s.opcode == 0x5a) && !e.trace.iter().any(|s| matches!(s.opcode, 0xf1 | 0xf4 | 0xfa)),
                "{name}: no external call executed"
            );
            let mut prefix = Expected::new(&pre);
            if allowance_prefix {
                prefix.store(mapping(55.into(), mapping(44.into(), 52.into())), 8.into());
                prefix.log(
                    vec![role("Approval(address,address,uint256)"), 44.into(), 55.into()],
                    vm::word(8.into()).to_vec(),
                );
            }
            prefix.check_prefix(name, &e)?;
            rollback(name, &e, &pre)?;
            ensure!(
                e.reads.iter().any(|v| v.key == 351.into() && v.value == 78.into()) && !e.reads.iter().any(|v| v.key == 301.into()),
                "{name}: pause before compliance"
            );
        }
        Ok(())
    }
}
