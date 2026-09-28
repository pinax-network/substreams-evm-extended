//! Exact captured SecuritiesToken execution in explicitly synthetic accounts.
//! Local role/getter expectations come from its captured OZ 5.3 source. This
//! neither executes proxy dispatch nor establishes deployed initialization.
use super::vm::{self, Execution, Exit, State};
use crate::ptoken_proof::cases::{call, execution_json, get, mapping, role, state_json, w};
use anyhow::{ensure, Context, Result};
use primitive_types::U256;
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn slot(s: &str) -> U256 {
    U256::from_str_radix(s, 16).unwrap()
}
pub fn role_root() -> U256 {
    slot("02dd7bc7dec4dceedda775e58dd541e08a116c6c53815c0bd028192f7b626800")
}
pub fn set_root() -> U256 {
    slot("c1f6fe24621ce81ec5827caf0253cadb74709b061630e6b55e82371705932000")
}
pub fn erc20_root() -> U256 {
    slot("52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace00")
}
pub fn initializer_slot() -> U256 {
    slot("f0c57e16840df040f15088dc2f81fe391c3923bec73e23a9662efc9c229c6a00")
}
pub fn member_key(r: U256, a: U256) -> U256 {
    mapping(a, mapping(r, role_root()))
}
pub fn admin_key(r: U256) -> U256 {
    mapping(r, role_root()) + U256::one()
}
pub fn head(r: U256) -> U256 {
    mapping(r, set_root())
}
pub fn element(r: U256, i: U256) -> U256 {
    vm::hash(&vm::word(head(r))).overflowing_add(i).0
}
pub fn position(r: U256, a: U256) -> U256 {
    mapping(a, head(r) + U256::one())
}
pub fn balance_key(a: U256) -> U256 {
    mapping(a, erc20_root())
}
pub fn allowance_key(a: U256, b: U256) -> U256 {
    mapping(b, mapping(a, erc20_root() + U256::one()))
}
fn account() -> U256 {
    U256::from_big_endian(&hex::decode(&super::CONTRACT[2..]).unwrap())
}
fn normalize(s: &State) -> State {
    s.iter().filter(|(_, v)| !v.is_zero()).map(|(k, v)| (*k, *v)).collect()
}
pub fn seed_role(s: &mut State, r: U256, members: &[U256]) {
    s.insert(head(r), members.len().into());
    for (i, m) in members.iter().enumerate() {
        s.insert(member_key(r, *m), 1.into());
        s.insert(element(r, i.into()), *m);
        s.insert(position(r, *m), (i + 1).into());
    }
}
pub fn base() -> State {
    let mut s = State::new();
    // Synthetic already-initialized storage; the initializer is not executed.
    s.insert(initializer_slot(), 1.into());
    seed_role(&mut s, 0.into(), &[1.into()]);
    seed_role(&mut s, role("UNRELATED_ROLE"), &[88.into()]);
    s.insert(balance_key(44.into()), 100.into());
    s.insert(balance_key(45.into()), 23.into());
    s.insert(erc20_root() + U256::from(2), 123.into());
    s.insert(allowance_key(44.into(), 55.into()), 9.into());
    // Unrelated client/UI/packed and reserved words must survive local roles.
    for (k, v) in [
        (0, U256::exp10(18)),
        (1, U256::exp10(18)),
        (2, U256::max_value()),
        (50, 77.into()),
        (100, 78.into()),
        (150, 257.into()),
        (149, 19.into()),
    ] {
        s.insert(U256::from(k as u64), v);
    }
    s
}
type Write = (U256, U256, U256);
fn writes(e: &Execution) -> Vec<Write> {
    e.writes.iter().map(|s| (s.key, s.old, s.new)).collect()
}
fn apply(s: &mut State, expected: &[Write]) -> Result<()> {
    for (key, old, new) in expected {
        ensure!(get(s, *key) == *old, "independent expected continuity");
        s.insert(*key, *new);
    }
    Ok(())
}
fn assert_state(name: &str, e: &Execution, pre: &State, expected: &[Write]) -> Result<()> {
    ensure!(writes(e) == expected, "{name}: ordered writes actual={:?}, expected={expected:?}", writes(e));
    let mut want = pre.clone();
    apply(&mut want, expected)?;
    ensure!(normalize(&e.committed) == normalize(&want), "{name}: complete committed storage");
    ensure!(e.committed_logs == e.logs, "{name}: complete committed logs");
    for h in &e.keccaks {
        ensure!(vm::hash(&h.input) == h.output, "{name}: preimage hash");
    }
    Ok(())
}
fn role_log(e: &Execution, r: U256, m: U256, caller: U256, grant: bool) -> Result<()> {
    let [log] = e.logs.as_slice() else {
        anyhow::bail!("expected one role log, got {}", e.logs.len());
    };
    ensure!(
        log.topics
            == vec![
                role(if grant {
                    "RoleGranted(bytes32,address,address)"
                } else {
                    "RoleRevoked(bytes32,address,address)"
                }),
                r,
                m,
                caller
            ]
            && log.data.is_empty(),
        "exact role event"
    );
    let first = e.writes.first().context("role log requires bool write")?;
    ensure!(first.step < log.step, "bool precedes role event");
    if let Some(second) = e.writes.get(1) {
        ensure!(log.step < second.step, "role event precedes set writes");
    }
    Ok(())
}
fn rollback(name: &str, e: &Execution, pre: &State, data: Vec<u8>, expected: &[Write]) -> Result<()> {
    ensure!(e.exit == Exit::Revert(data), "{name}: exact revert {:?}", e.exit);
    ensure!(
        writes(e) == expected && e.committed == *pre && e.committed_logs.is_empty(),
        "{name}: attempted writes and full rollback"
    );
    Ok(())
}
fn set_effects(r: U256, before: &[U256], m: U256, grant: bool) -> (Vec<U256>, Vec<Write>) {
    let mut after = before.to_vec();
    let mut result = vec![];
    if grant {
        if !before.contains(&m) {
            after.push(m);
            result.extend([
                (head(r), before.len().into(), after.len().into()),
                (element(r, before.len().into()), 0.into(), m),
                (position(r, m), 0.into(), after.len().into()),
            ]);
        }
    } else if let Some(i) = before.iter().position(|a| *a == m) {
        let last = before.len() - 1;
        let tail = before[last];
        after.swap_remove(i);
        if i != last {
            result.extend([(element(r, i.into()), m, tail), (position(r, tail), before.len().into(), (i + 1).into())]);
        }
        // This exact ordering is an expectation to confirm against the selected
        // 0.8.24 runtime, not a transfer of another compiler's qualification.
        result.extend([
            (element(r, last.into()), tail, 0.into()),
            (head(r), before.len().into(), after.len().into()),
            (position(r, m), (i + 1).into(), 0.into()),
        ]);
    }
    (after, result)
}
#[derive(Clone, Copy)]
enum Operation {
    Grant,
    Revoke,
    Renounce,
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
        size: usize,
        pre: &State,
        scope: &str,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<Execution> {
        let e = vm::execute_with_self_code_size(code, data, caller, account(), pre, Some(size));
        self.calls += 1;
        let row = json!({"name":name,"signature":sig,"scope":scope,"caller":w(caller),"address":w(account()),"self_code_size":size,"calldata":hex::encode(data),"prestate":state_json(pre),"execution":execution_json(&e)});
        save(self.calls, &row)?;
        self.cases.push(row);
        if scope != "unsupported_path_control" {
            ensure!(!matches!(e.exit, Exit::HarnessFailure(_) | Exit::Invalid), "{name}: unexpected {:?}", e.exit);
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
        self.record(name, sig, self.runtime, &call(sig, args), caller, 10836, pre, "local_operation", save)
    }
    fn getter(&mut self, name: &str, sig: &str, args: &[U256], want: Vec<u8>, pre: &State, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let e = self.run(name, sig, args, 9.into(), pre, save)?;
        ensure!(
            e.exit == Exit::Return(want) && e.committed == *pre && e.writes.is_empty() && e.logs.is_empty(),
            "{name}: exact readonly {sig}: {:?}",
            e.exit
        );
        Ok(())
    }
    fn verify_role(
        &mut self,
        name: &str,
        r: U256,
        members: &[U256],
        checked: U256,
        s: &State,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<()> {
        self.getter(name, "getRoleMemberCount(bytes32)", &[r], vm::word(members.len().into()).to_vec(), s, save)?;
        self.getter(name, "getRoleAdmin(bytes32)", &[r], vm::word(get(s, admin_key(r))).to_vec(), s, save)?;
        let mut all = vm::word(32.into()).to_vec();
        all.extend(vm::word(members.len().into()));
        for (i, m) in members.iter().enumerate() {
            all.extend(vm::word(*m));
            ensure!(get(s, position(r, *m)) == (i + 1).into(), "{name}: independent member index");
            self.getter(name, "getRoleMember(bytes32,uint256)", &[r, i.into()], vm::word(*m).to_vec(), s, save)?;
        }
        self.getter(name, "getRoleMembers(bytes32)", &[r], all, s, save)?;
        for m in members.iter().copied().chain([checked, 0.into(), 99.into()]).collect::<BTreeSet<_>>() {
            self.getter(
                name,
                "hasRole(bytes32,address)",
                &[r, m],
                vm::word(u8::from(members.contains(&m)).into()).to_vec(),
                s,
                save,
            )?;
        }
        for index in [members.len().into(), U256::max_value()] {
            let e = self.run(name, "getRoleMember(bytes32,uint256)", &[r, index], 9.into(), s, save)?;
            rollback(name, &e, s, call("Panic(uint256)", &[0x32.into()]), &[])?;
            ensure!(e.logs.is_empty(), "bounds failure logs");
        }
        Ok(())
    }
    fn balances(&mut self, name: &str, s: &State, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        for (sig, args, want) in [
            ("balanceOf(address)", vec![44.into()], 100u64),
            ("balanceOf(address)", vec![45.into()], 23),
            ("balanceOf(address)", vec![46.into()], 0),
            ("totalSupply()", vec![], 123),
            ("allowance(address,address)", vec![44.into(), 55.into()], 9),
            ("allowance(address,address)", vec![55.into(), 44.into()], 0),
        ] {
            self.getter(name, sig, &args, vm::word(want.into()).to_vec(), s, save)?;
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn operation(
        &mut self,
        name: &str,
        pre: &State,
        r: U256,
        before: &[U256],
        m: U256,
        op: Operation,
        caller: U256,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<State> {
        let grant = matches!(op, Operation::Grant);
        let sig = match op {
            Operation::Grant => "grantRole(bytes32,address)",
            Operation::Revoke => "revokeRole(bytes32,address)",
            Operation::Renounce => "renounceRole(bytes32,address)",
        };
        let e = self.run(name, sig, &[r, m], caller, pre, save)?;
        ensure!(e.exit == Exit::Return(vec![]), "{name}: local success {:?}", e.exit);
        let (after, set) = set_effects(r, before, m, grant);
        let changed = before.contains(&m) != grant;
        let mut expected = vec![];
        if changed {
            expected.push((member_key(r, m), u8::from(!grant).into(), u8::from(grant).into()));
            expected.extend(set);
            role_log(&e, r, m, caller, grant)?;
        } else {
            ensure!(e.logs.is_empty(), "{name}: no-op emits no role event");
        }
        assert_state(name, &e, pre, &expected)?;
        if changed {
            for input in [
                [vm::word(r), vm::word(role_root())].concat(),
                [vm::word(m), vm::word(mapping(r, role_root()))].concat(),
                [vm::word(r), vm::word(set_root())].concat(),
                [vm::word(m), vm::word(head(r) + U256::one())].concat(),
                vm::word(head(r)).to_vec(),
            ] {
                ensure!(e.keccaks.iter().any(|h| h.input == input), "{name}: actual namespaced linked preimage");
            }
        }
        self.verify_role(name, r, &after, m, &e.committed, save)?;
        self.balances(name, &e.committed, save)?;
        Ok(e.committed)
    }
    pub fn constructor(&mut self, creation: &[u8], save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let pre = State::new();
        let e = self.record(
            "synthetic_implementation_constructor",
            "constructor()",
            creation,
            &[],
            9.into(),
            0,
            &pre,
            "synthetic_constructor_only",
            save,
        )?;
        ensure!(
            e.exit == Exit::Return(self.runtime.to_vec()),
            "synthetic constructor returns exact runtime {:?}",
            e.exit
        );
        assert_state("synthetic constructor", &e, &pre, &[(initializer_slot(), 0.into(), U256::from(u64::MAX))])?;
        let [log] = e.logs.as_slice() else {
            anyhow::bail!("constructor requires one Initialized log");
        };
        ensure!(
            log.topics == vec![role("Initialized(uint64)")] && log.data == vm::word(U256::from(u64::MAX)) && e.writes[0].step < log.step,
            "exact constructor lock event"
        );
        Ok(())
    }
    pub fn matrix(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let r = role("MATRIX_ROLE");
        for (name, before, member, grant) in [
            ("grant_empty", vec![], 3u64, true),
            ("grant_nonempty", vec![3, 4], 5, true),
            ("grant_zero_empty", vec![], 0, true),
            ("grant_zero_nonempty", vec![3, 4], 0, true),
            ("grant_duplicate", vec![3, 4], 3, true),
            ("grant_duplicate_zero", vec![0, 3], 0, true),
            ("revoke_first", vec![3, 4, 5], 3, false),
            ("revoke_middle", vec![3, 4, 5], 4, false),
            ("revoke_tail", vec![3, 4, 5], 5, false),
            ("revoke_sole", vec![3], 3, false),
            ("revoke_zero_tail_swap", vec![3, 4, 0], 4, false),
            ("revoke_zero_first", vec![0, 3, 4], 0, false),
            ("revoke_zero_middle", vec![3, 0, 4], 0, false),
            ("revoke_zero_tail", vec![3, 4, 0], 0, false),
            ("revoke_zero_sole", vec![0], 0, false),
            ("revoke_absent_empty", vec![], 3, false),
            ("revoke_absent", vec![3, 4], 5, false),
        ] {
            let before: Vec<_> = before.into_iter().map(|v| U256::from(v as u64)).collect();
            let mut pre = base();
            seed_role(&mut pre, r, &before);
            self.operation(
                name,
                &pre,
                r,
                &before,
                member.into(),
                if grant { Operation::Grant } else { Operation::Revoke },
                1.into(),
                save,
            )?;
        }
        for (name, r) in [
            ("default_admin", U256::zero()),
            ("issuer_role", role("ISSUER_ROLE")),
            ("full_width_role", U256::max_value()),
        ] {
            let mut pre = base();
            let before = if r.is_zero() { vec![1.into()] } else { vec![] };
            seed_role(&mut pre, r, &before);
            self.operation(name, &pre, r, &before, (U256::one() << 160) - U256::one(), Operation::Grant, 1.into(), save)?;
        }
        for (name, before, m) in [
            ("renounce_present", vec![3.into(), 4.into()], 3.into()),
            ("renounce_absent", vec![4.into()], 3.into()),
            ("renounce_zero", vec![0.into()], 0.into()),
        ] {
            let mut pre = base();
            seed_role(&mut pre, r, &before);
            self.operation(name, &pre, r, &before, m, Operation::Renounce, m, save)?;
        }
        self.admin_controls(save)?;
        self.sequences(save)?;
        self.failures(save)?;
        self.malformed(save)?;
        self.raw_getters(save)?;
        self.exclusions(save)?;
        Ok(())
    }
    fn admin_controls(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let r = role("ARBITRARY_ADMIN_TARGET");
        for (name, admin) in [
            ("full_width_admin", U256::max_value()),
            ("self_admin", r),
            ("high_bit_admin", U256::one() << 255),
        ] {
            let mut pre = base();
            seed_role(&mut pre, admin, &[7.into()]);
            pre.insert(admin_key(r), admin);
            let before = if admin == r { vec![7.into()] } else { vec![] };
            let mut next = before.clone();
            next.push(3.into());
            let post = self.operation(name, &pre, r, &before, 3.into(), Operation::Grant, 7.into(), save)?;
            self.operation(&format!("{name}_revoke"), &post, r, &next, 3.into(), Operation::Revoke, 7.into(), save)?;
            for (suffix, sig) in [("denied_grant", "grantRole(bytes32,address)"), ("denied_revoke", "revokeRole(bytes32,address)")] {
                let e = self.run(&format!("{name}_{suffix}"), sig, &[r, 3.into()], 1.into(), &pre, save)?;
                rollback(
                    name,
                    &e,
                    &pre,
                    call("AccessControlUnauthorizedAccount(address,bytes32)", &[1.into(), admin]),
                    &[],
                )?;
                ensure!(e.logs.is_empty(), "admin refusal has no log");
            }
        }
        Ok(())
    }
    fn sequences(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let roles = [role("SEQUENCE_A"), role("SEQUENCE_B")];
        let mut members = [vec![], vec![]];
        let mut state = base();
        for (i, (grant, m)) in [(true, 3u64), (true, 0), (true, 4), (false, 3), (true, 3), (false, 0), (false, 4), (false, 3)]
            .into_iter()
            .enumerate()
        {
            for (j, r) in roles.into_iter().enumerate() {
                state = self.operation(
                    &format!("sequence_{i}_{j}"),
                    &state,
                    r,
                    &members[j],
                    m.into(),
                    if grant { Operation::Grant } else { Operation::Revoke },
                    1.into(),
                    save,
                )?;
                members[j] = set_effects(r, &members[j], m.into(), grant).0;
            }
        }
        Ok(())
    }
    fn failures(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let r = role("MATRIX_ROLE");
        let pre = base();
        for (name, sig, args, caller, data) in [
            (
                "unauthorized_grant",
                "grantRole(bytes32,address)",
                vec![r, 3.into()],
                9,
                call("AccessControlUnauthorizedAccount(address,bytes32)", &[9.into(), 0.into()]),
            ),
            (
                "unauthorized_revoke",
                "revokeRole(bytes32,address)",
                vec![r, 3.into()],
                9,
                call("AccessControlUnauthorizedAccount(address,bytes32)", &[9.into(), 0.into()]),
            ),
            (
                "wrong_renounce_confirmation",
                "renounceRole(bytes32,address)",
                vec![r, 3.into()],
                9,
                call("AccessControlBadConfirmation()", &[]),
            ),
            ("unauthorized_mint", "mint(uint256)", vec![1.into()], 9, call("UnauthorizedRole()", &[])),
            ("unauthorized_burn", "burn(uint256)", vec![1.into()], 9, call("UnauthorizedRole()", &[])),
            (
                "zero_compliance",
                "setCompliance(address)",
                vec![0.into()],
                1,
                call("ComplianceZeroAddress()", &[]),
            ),
            (
                "zero_pause_manager",
                "setPauseManager(address)",
                vec![0.into()],
                1,
                call("PauseManagerCantBeZero()", &[]),
            ),
        ] {
            let e = self.run(name, sig, &args, U256::from(caller as u64), &pre, save)?;
            rollback(name, &e, &pre, data, &[])?;
            ensure!(e.logs.is_empty(), "{name}: no logs");
        }
        let mut disabled = pre.clone();
        disabled.insert(150.into(), 0.into());
        for (name, sig, data) in [
            ("mint_disabled", "mint(uint256)", call("MintDisabled()", &[])),
            ("burn_disabled", "burn(uint256)", call("BurnDisabled()", &[])),
        ] {
            let e = self.run(name, sig, &[1.into()], 1.into(), &disabled, save)?;
            rollback(name, &e, &disabled, data, &[])?;
            ensure!(e.logs.is_empty(), "disabled before clients");
        }
        for (name, data) in [
            ("bad_address", call("grantRole(bytes32,address)", &[r, U256::one() << 160])),
            ("bad_renounce_address", call("renounceRole(bytes32,address)", &[r, U256::one() << 160])),
            ("short_abi", call("grantRole(bytes32,address)", &[r])),
            ("unknown_selector", vec![0xff; 4]),
        ] {
            let e = self.record(name, "malformed ABI", self.runtime, &data, 1.into(), 10836, &pre, "local_operation", save)?;
            rollback(name, &e, &pre, vec![], &[])?;
            ensure!(e.logs.is_empty(), "ABI refusal before effects");
        }
        Ok(())
    }
    fn malformed(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let r = role("MALFORMED_ROLE");
        let m = U256::from(3);
        // OZ 5.3 invokes the set only if the boolean actually changes. All four
        // cells are outside coherent-state admission, including successful ones.
        for (name, b, indexed, grant) in [
            ("malformed_true_no_index_grant", true, false, true),
            ("malformed_false_index_grant", false, true, true),
            ("malformed_true_no_index_revoke", true, false, false),
            ("malformed_false_index_revoke", false, true, false),
        ] {
            let mut pre = base();
            if indexed {
                seed_role(&mut pre, r, &[m, 4.into()]);
            }
            pre.insert(member_key(r, m), u8::from(b).into());
            let e = self.run(
                name,
                if grant { "grantRole(bytes32,address)" } else { "revokeRole(bytes32,address)" },
                &[r, m],
                1.into(),
                &pre,
                save,
            )?;
            ensure!(e.exit == Exit::Return(vec![]), "malformed-state return");
            let expected = if b != grant {
                vec![(member_key(r, m), u8::from(b).into(), u8::from(grant).into())]
            } else {
                vec![]
            };
            assert_state(name, &e, &pre, &expected)?;
            if b != grant {
                role_log(&e, r, m, 1.into(), grant)?;
            } else {
                ensure!(e.logs.is_empty(), "bool gate skips set and log");
            }
        }
        for (name, index, len, panic) in [
            ("empty_array_prefix_revert", U256::one(), 0u64, 0x11),
            ("out_of_bounds_prefix_revert", 3.into(), 1, 0x32),
            ("max_index_prefix_revert", U256::max_value(), 1, 0x32),
        ] {
            let mut pre = base();
            pre.insert(member_key(r, m), 1.into());
            pre.insert(position(r, m), index);
            pre.insert(head(r), len.into());
            pre.insert(element(r, 0.into()), 4.into());
            let e = self.run(name, "revokeRole(bytes32,address)", &[r, m], 1.into(), &pre, save)?;
            rollback(
                name,
                &e,
                &pre,
                call("Panic(uint256)", &[U256::from(panic as u64)]),
                &[(member_key(r, m), 1.into(), 0.into())],
            )?;
            role_log(&e, r, m, 1.into(), false)?;
        }
        let mut pre = base();
        pre.insert(head(r), U256::max_value());
        let e = self.run("max_length_push_outside_domain", "grantRole(bytes32,address)", &[r, m], 1.into(), &pre, save)?;
        ensure!(e.exit == Exit::Return(vec![]), "selected compiler maximum-length outcome {:?}", e.exit);
        assert_state(
            "max length",
            &e,
            &pre,
            &[
                (member_key(r, m), 0.into(), 1.into()),
                (head(r), U256::max_value(), 0.into()),
                (element(r, U256::max_value()), 0.into(), m),
                (position(r, m), 0.into(), 0.into()),
            ],
        )?;
        role_log(&e, r, m, 1.into(), true)?;
        for (name, old, new) in [
            ("dirty_low_bool_revoke", U256::from(2), U256::zero()),
            ("dirty_high_bool_revoke", (U256::one() << 200) + U256::one(), U256::one() << 200),
        ] {
            let mut pre = base();
            seed_role(&mut pre, r, &[m]);
            pre.insert(member_key(r, m), old);
            let e = self.run(name, "revokeRole(bytes32,address)", &[r, m], 1.into(), &pre, save)?;
            ensure!(e.exit == Exit::Return(vec![]), "dirty bool outcome");
            let mut expected = vec![(member_key(r, m), old, new)];
            expected.extend(set_effects(r, &[m], m, false).1);
            assert_state(name, &e, &pre, &expected)?;
            role_log(&e, r, m, 1.into(), false)?;
            self.getter(name, "hasRole(bytes32,address)", &[r, m], vm::word(0.into()).to_vec(), &e.committed, save)?;
        }
        // The runtime does not read the moved tail's bool; synthetic inconsistent
        // membership is preserved, not evidence of coherent admitted state.
        let mut pre = base();
        seed_role(&mut pre, r, &[m, 4.into()]);
        pre.insert(member_key(r, 4.into()), 0.into());
        let e = self.run(
            "moved_tail_false_bool_outside_domain",
            "revokeRole(bytes32,address)",
            &[r, m],
            1.into(),
            &pre,
            save,
        )?;
        let mut expected = vec![(member_key(r, m), 1.into(), 0.into())];
        expected.extend(set_effects(r, &[m, 4.into()], m, false).1);
        ensure!(e.exit == Exit::Return(vec![]), "malformed moved-tail success");
        assert_state("moved tail", &e, &pre, &expected)?;
        role_log(&e, r, m, 1.into(), false)?;
        ensure!(
            !e.reads.iter().any(|read| read.key == member_key(r, 4.into())),
            "no invented moved-tail bool read"
        );
        Ok(())
    }
    fn raw_getters(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        for (i, value) in [U256::zero(), U256::one(), 123.into(), U256::max_value()].into_iter().enumerate() {
            let mut pre = base();
            pre.insert(balance_key(44.into()), value);
            pre.insert(balance_key(45.into()), 0.into());
            pre.insert(erc20_root() + U256::from(2), value);
            pre.insert(allowance_key(44.into(), 55.into()), value);
            // Vary UI/client fields independently: raw getters remain local.
            for key in [0u64, 1, 2, 50, 100, 150] {
                pre.insert(key.into(), if i % 2 == 0 { U256::zero() } else { U256::max_value() });
            }
            for (sig, args, want) in [
                ("balanceOf(address)", vec![44.into()], value),
                ("balanceOf(address)", vec![45.into()], 0.into()),
                ("totalSupply()", vec![], value),
                ("allowance(address,address)", vec![44.into(), 55.into()], value),
                ("allowance(address,address)", vec![55.into(), 44.into()], 0.into()),
            ] {
                self.getter(&format!("raw_getters_{i}"), sig, &args, vm::word(want).to_vec(), &pre, save)?;
            }
        }
        Ok(())
    }
    fn exclusions(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let pre = base();
        let init = initializer_data();
        let empty = State::new();
        for (name, sig, data, state, expected) in [
            (
                "excluded_initializer",
                "initialize(string,string,string,address,address,address,address[])",
                init,
                &empty,
                "unsupported opcode 0x42",
            ),
            (
                "excluded_set_compliance",
                "setCompliance(address)",
                call("setCompliance(address)", &[77.into()]),
                &pre,
                "EXTCODESIZE unknown external account",
            ),
            (
                "excluded_set_pause_manager",
                "setPauseManager(address)",
                call("setPauseManager(address)", &[78.into()]),
                &pre,
                "EXTCODESIZE unknown external account",
            ),
            (
                "excluded_transfer",
                "transfer(address,uint256)",
                call("transfer(address,uint256)", &[45.into(), 1.into()]),
                &pre,
                // GAS precedes the pause-manager STATICCALL. The external
                // call is not executed by this bounded host proof.
                "unsupported opcode 0x5a at pc 7368",
            ),
            (
                "excluded_mint",
                "mint(uint256)",
                call("mint(uint256)", &[1.into()]),
                &pre,
                "unsupported opcode 0x5a at pc 7368",
            ),
            (
                "excluded_burn",
                "burn(uint256)",
                call("burn(uint256)", &[1.into()]),
                &pre,
                "unsupported opcode 0x5a at pc 7368",
            ),
            (
                "excluded_balance_ui",
                "balanceOfUI(address)",
                call("balanceOfUI(address)", &[44.into()]),
                &pre,
                "unsupported opcode 0x42",
            ),
            (
                "excluded_ui_schedule",
                "setUIMultiplier(uint256,uint256)",
                call("setUIMultiplier(uint256,uint256)", &[U256::exp10(18), 100.into()]),
                &pre,
                "unsupported opcode 0x42",
            ),
        ] {
            let e = self.record(name, sig, self.runtime, &data, 1.into(), 10836, state, "unsupported_path_control", save)?;
            ensure!(
                matches!(&e.exit,Exit::HarnessFailure(message) if message.contains(expected)),
                "{name}: expected explicit unsupported path, got {:?}",
                e.exit
            );
            ensure!(e.committed == *state && e.committed_logs.is_empty(), "{name}: unsupported path commits nothing");
        }
        Ok(())
    }
}
/// Synthetic valid ABI, not captured deployment arguments. The path must stop
/// at an unsupported timestamp/client operation before initialization succeeds.
pub fn initializer_data() -> Vec<u8> {
    let mut data = call(
        "initialize(string,string,string,address,address,address,address[])",
        &[224.into(), 288.into(), 352.into(), 77.into(), 78.into(), 1.into(), 416.into()],
    );
    for text in [b"Security".as_slice(), b"SEC".as_slice(), b"SYNTHETIC".as_slice()] {
        data.extend(vm::word(text.len().into()));
        data.extend(text);
        data.resize(4 + (data.len() - 4).div_ceil(32) * 32, 0);
    }
    data.extend(vm::word(1.into()));
    data.extend(vm::word(2.into()));
    data
}
