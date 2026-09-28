//! Synthetic coherent accounts executed by the exact selected runtime.
//! Expected sets/balances are independent host data; no production admission.
use super::vm::{self, Execution, Exit, State};
use anyhow::{ensure, Context, Result};
use primitive_types::U256;
use serde_json::{json, Value};
use std::collections::BTreeSet;
pub fn w(v: U256) -> String {
    format!("0x{}", hex::encode(vm::word(v)))
}
pub fn mapping(key: U256, root: U256) -> U256 {
    vm::hash(&[vm::word(key), vm::word(root)].concat())
}
pub fn role(name: &str) -> U256 {
    vm::hash(name.as_bytes())
}
pub fn call(signature: &str, args: &[U256]) -> Vec<u8> {
    let mut data = vm::word(vm::hash(signature.as_bytes()))[..4].to_vec();
    for arg in args {
        data.extend(vm::word(*arg));
    }
    data
}
pub fn get(state: &State, key: U256) -> U256 {
    state.get(&key).copied().unwrap_or_default()
}
fn normalize(state: &State) -> State {
    state.iter().filter(|(_, v)| !v.is_zero()).map(|(k, v)| (*k, *v)).collect()
}
pub fn member_key(role: U256, member: U256) -> U256 {
    mapping(member, mapping(role, 5.into()))
}
pub fn head(role: U256) -> U256 {
    mapping(role, 6.into())
}
pub fn element(role: U256, index: usize) -> U256 {
    vm::hash(&vm::word(head(role))).overflowing_add(index.into()).0
}
pub fn position(role: U256, member: U256) -> U256 {
    mapping(member, head(role).overflowing_add(1.into()).0)
}
pub fn set_role(state: &mut State, role: U256, members: &[U256]) {
    state.insert(head(role), members.len().into());
    for (i, member) in members.iter().enumerate() {
        state.insert(member_key(role, *member), 1.into());
        state.insert(element(role, i), *member);
        state.insert(position(role, *member), (i + 1).into());
    }
}
fn base() -> State {
    let mut s = State::new();
    set_role(&mut s, 0.into(), &[1.into()]);
    s
}
pub fn state_json(s: &State) -> Value {
    Value::Array(s.iter().map(|(k, v)| json!({"key":w(*k),"value":w(*v)})).collect())
}
pub fn execution_json(e: &Execution) -> Value {
    let exit = match &e.exit {
        Exit::Return(b) => json!({"kind":"return","data":hex::encode(b)}),
        Exit::Revert(b) => json!({"kind":"revert","data":hex::encode(b)}),
        Exit::Invalid => json!({"kind":"invalid"}),
        Exit::HarnessFailure(s) => json!({"kind":"harness_failure","error":s}),
    };
    json!({"exit":exit,"committed_storage":state_json(&e.committed),"committed_logs":e.committed_logs.len(),
    "writes":e.writes.iter().map(|s|json!({"step":s.step,"pc":s.pc,"key":w(s.key),"old":w(s.old),"new":w(s.new)})).collect::<Vec<_>>(),
    "reads":e.reads.iter().map(|s|json!({"step":s.step,"pc":s.pc,"key":w(s.key),"value":w(s.value)})).collect::<Vec<_>>(),
    "logs":e.logs.iter().map(|s|json!({"step":s.step,"pc":s.pc,"topics":s.topics.iter().map(|v|w(*v)).collect::<Vec<_>>(),"data":hex::encode(&s.data)})).collect::<Vec<_>>(),
    "keccaks":e.keccaks.iter().map(|s|json!({"step":s.step,"pc":s.pc,"input":hex::encode(&s.input),"output":w(s.output)})).collect::<Vec<_>>(),
    "trace":e.trace.iter().map(|s|json!({"pc":s.pc,"opcode":s.opcode,"stack_top":s.stack_top.iter().map(|v|w(*v)).collect::<Vec<_>>()})).collect::<Vec<_>>()})
}
#[derive(Clone, Copy)]
enum RoleOperation {
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
    fn run(
        &mut self,
        name: &str,
        signature: &str,
        args: &[U256],
        caller: U256,
        prestate: &State,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<Execution> {
        let data = call(signature, args);
        let e = vm::execute(
            self.runtime,
            &data,
            caller,
            U256::from_big_endian(&hex::decode(&super::CONTRACT[2..])?),
            prestate,
        );
        self.calls += 1;
        let record = json!({"name":name,"signature":signature,"caller":w(caller),"calldata":hex::encode(data),"prestate":state_json(prestate),"execution":execution_json(&e)});
        save(self.calls, &record)?; // Persist the entire attempted trace before any expectation can fail.
        self.cases.push(record);
        ensure!(!matches!(e.exit, Exit::HarnessFailure(_) | Exit::Invalid), "{name}: {:?}", e.exit);
        Ok(e)
    }
    fn getter(
        &mut self,
        name: &str,
        signature: &str,
        args: &[U256],
        expected: U256,
        state: &State,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<()> {
        let e = self.run(name, signature, args, 9.into(), state, save)?;
        ensure!(
            e.exit == Exit::Return(vm::word(expected).to_vec()),
            "{name}: getter {:?}, expected {}",
            e.exit,
            w(expected)
        );
        ensure!(e.writes.is_empty() && e.logs.is_empty() && e.committed == *state, "getter mutated");
        Ok(())
    }
    fn verify_set(
        &mut self,
        name: &str,
        r: U256,
        members: &[U256],
        removed: U256,
        state: &State,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<()> {
        self.getter(name, "getRoleMemberCount(bytes32)", &[r], members.len().into(), state, save)?;
        let e = self.run(name, "getRoleMembers(bytes32)", &[r], 9.into(), state, save)?;
        let mut data = vm::word(32.into()).to_vec();
        data.extend(vm::word(members.len().into()));
        for m in members {
            data.extend(vm::word(*m));
        }
        ensure!(
            e.exit == Exit::Return(data) && e.writes.is_empty() && e.logs.is_empty(),
            "full set getter including MCOPY"
        );
        for (i, m) in members.iter().enumerate() {
            self.getter(name, "getRoleMember(bytes32,uint256)", &[r, i.into()], *m, state, save)?;
            ensure!(get(state, position(r, *m)) == (i + 1).into(), "index coherence");
        }
        for m in members.iter().copied().chain([removed, 0.into(), 99.into()]).collect::<BTreeSet<_>>() {
            self.getter(name, "hasRole(bytes32,address)", &[r, m], u8::from(members.contains(&m)).into(), state, save)?;
        }
        let e = self.run(name, "getRoleMember(bytes32,uint256)", &[r, members.len().into()], 9.into(), state, save)?;
        ensure!(e.exit == Exit::Revert(call("Panic(uint256)", &[0x32.into()])), "array bound revert");
        Ok(())
    }
    fn role_operation(
        &mut self,
        name: &str,
        r: U256,
        before: &[U256],
        member: U256,
        operation: RoleOperation,
        save: &mut impl FnMut(usize, &Value) -> Result<()>,
    ) -> Result<()> {
        let grant = matches!(operation, RoleOperation::Grant);
        let renounce = matches!(operation, RoleOperation::Renounce);
        let mut pre = base();
        set_role(&mut pre, r, before);
        // Unrelated role, balance, allowance and packed pause/scalar cells survive.
        let other = role("UNRELATED_ROLE");
        set_role(&mut pre, other, &[88.into()]);
        pre.insert(mapping(44.into(), 0.into()), 123.into());
        pre.insert(2.into(), 123.into());
        pre.insert(mapping(55.into(), mapping(44.into(), 1.into())), 9.into());
        let mut after = before.to_vec();
        let index = before.iter().position(|v| *v == member);
        let changed = if grant {
            if index.is_none() {
                after.push(member);
                true
            } else {
                false
            }
        } else if let Some(i) = index {
            after.swap_remove(i);
            true
        } else {
            false
        };
        let signature = if grant {
            "grantRole(bytes32,address)"
        } else if renounce {
            "renounceRole(bytes32,address)"
        } else {
            "revokeRole(bytes32,address)"
        };
        let caller = if renounce { member } else { 1.into() };
        let e = self.run(name, signature, &[r, member], caller, &pre, save)?;
        ensure!(e.exit == Exit::Return(vec![]), "{name} success");
        let mut expected = pre.clone();
        let mut writes = vec![];
        if changed {
            writes.push((member_key(r, member), if grant { 0.into() } else { 1.into() }, u8::from(grant).into()));
            if grant {
                writes.push((head(r), before.len().into(), after.len().into()));
                writes.push((element(r, before.len()), 0.into(), member));
                writes.push((position(r, member), 0.into(), after.len().into()));
            } else {
                let i = index.unwrap();
                let last = before.len() - 1;
                let tail = before[last];
                if i != last {
                    writes.push((element(r, i), member, tail));
                    writes.push((position(r, tail), before.len().into(), (i + 1).into()));
                }
                // The selected compiler clears the popped cell before decrementing length.
                writes.push((element(r, last), tail, 0.into()));
                writes.push((head(r), before.len().into(), after.len().into()));
                writes.push((position(r, member), (i + 1).into(), 0.into()));
            }
            for (k, old, new) in &writes {
                ensure!(get(&expected, *k) == *old, "independent prestate continuity");
                expected.insert(*k, *new);
            }
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
                            member,
                            caller
                        ]
                    && e.logs[0].data.is_empty(),
                "{name} role log"
            );
            ensure!(
                e.writes.len() >= 2 && e.writes[0].step < e.logs[0].step && e.logs[0].step < e.writes[1].step,
                "{name}: bool/log/set order"
            );
        } else {
            ensure!(e.logs.is_empty(), "no-op log");
        }
        let observed: Vec<_> = e.writes.iter().map(|x| (x.key, x.old, x.new)).collect();
        ensure!(observed == writes, "{name}: ordered writes differ\nobserved={observed:?}\nexpected={writes:?}");
        ensure!(normalize(&e.committed) == normalize(&expected), "{name}: exact all-cell state");
        ensure!(e.committed_logs == e.logs, "committed role logs");
        if changed {
            for preimage in [
                [vm::word(r), vm::word(5.into())].concat(),
                [vm::word(member), vm::word(mapping(r, 5.into()))].concat(),
                [vm::word(r), vm::word(6.into())].concat(),
                [vm::word(member), vm::word(head(r).overflowing_add(1.into()).0)].concat(),
                vm::word(head(r)).to_vec(),
            ] {
                ensure!(
                    e.keccaks.iter().any(|h| h.input == preimage),
                    "{name}: actual linked mapping/array preimage missing"
                );
            }
        }
        for hash in &e.keccaks {
            ensure!(vm::hash(&hash.input) == hash.output, "actual preimage");
        }
        self.verify_set(name, r, &after, member, &e.committed, save)?;
        self.getter(name, "balanceOf(address)", &[44.into()], 123.into(), &e.committed, save)?;
        self.getter(name, "totalSupply()", &[], 123.into(), &e.committed, save)?;
        self.getter(name, "getRoleMemberCount(bytes32)", &[other], 1.into(), &e.committed, save)?;
        Ok(())
    }
    pub fn matrix(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let r = role("MATRIX_ROLE");
        for (name, before, member, grant) in [
            ("grant_empty", vec![], 2, true),
            ("grant_nonempty", vec![2, 3], 4, true),
            ("grant_zero_empty", vec![], 0, true),
            ("grant_zero_nonempty", vec![2, 3], 0, true),
            ("grant_duplicate", vec![2, 3], 2, true),
            ("grant_duplicate_zero", vec![0, 3], 0, true),
            ("remove_first", vec![2, 3, 4], 2, false),
            ("remove_middle", vec![2, 3, 4], 3, false),
            ("remove_tail", vec![2, 3, 4], 4, false),
            ("remove_only", vec![2], 2, false),
            ("remove_middle_zero_tail", vec![2, 3, 0], 3, false),
            ("remove_zero_first", vec![0, 2, 3], 0, false),
            ("remove_zero_middle", vec![2, 0, 3], 0, false),
            ("remove_zero_tail", vec![2, 3, 0], 0, false),
            ("remove_only_zero", vec![0], 0, false),
            ("remove_absent_empty", vec![], 2, false),
            ("remove_absent_nonempty", vec![2, 3], 4, false),
        ] {
            self.role_operation(
                name,
                r,
                &before.iter().map(|v| U256::from(*v as u64)).collect::<Vec<_>>(),
                (member as u64).into(),
                if grant { RoleOperation::Grant } else { RoleOperation::Revoke },
                save,
            )?;
        }
        for (name, r) in [
            ("grant_default_admin", U256::zero()),
            ("grant_pauser", role("PAUSER_ROLE")),
            ("grant_minter", role("MINTER_ROLE")),
            ("grant_arbitrary", U256::max_value()),
        ] {
            let before = if r.is_zero() { vec![1.into()] } else { vec![] };
            self.role_operation(name, r, &before, 2.into(), RoleOperation::Grant, save)?;
        }
        self.role_operation("renounce_present", r, &[2.into(), 3.into()], 2.into(), RoleOperation::Renounce, save)?;
        self.role_operation("renounce_absent", r, &[3.into()], 2.into(), RoleOperation::Renounce, save)?;
        for (name, sig, args, caller, error) in [
            (
                "unauthorized_grant",
                "grantRole(bytes32,address)",
                vec![r, 2.into()],
                9.into(),
                call("AccessControlUnauthorizedAccount(address,bytes32)", &[9.into(), 0.into()]),
            ),
            (
                "unauthorized_revoke",
                "revokeRole(bytes32,address)",
                vec![r, 2.into()],
                9.into(),
                call("AccessControlUnauthorizedAccount(address,bytes32)", &[9.into(), 0.into()]),
            ),
            (
                "renounce_wrong_confirmation",
                "renounceRole(bytes32,address)",
                vec![r, 2.into()],
                9.into(),
                call("AccessControlBadConfirmation()", &[]),
            ),
        ] {
            let pre = base();
            let e = self.run(name, sig, &args, caller, &pre, save)?;
            ensure!(
                e.exit == Exit::Revert(error) && e.writes.is_empty() && e.logs.is_empty() && e.committed == pre,
                "{name}: exact failure"
            );
        }
        self.sequence_and_failure_controls(save)?;
        self.balance_matrix(save)?;
        Ok(())
    }
    pub fn constructor(&mut self, creation: &[u8], save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let address = U256::from_big_endian(&hex::decode(&super::CONTRACT[2..])?);
        let deployer = U256::from_big_endian(&hex::decode("59eac6c267f4c01ce924caf4f01e9673d43c50fd")?);
        let admin = U256::from_big_endian(&hex::decode("bf29536989200077547c3270e3f48e729f7880f8")?);
        let e = vm::execute(creation, &[], deployer, address, &State::new());
        self.calls += 1;
        let record = json!({"name":"captured_creation_with_exact_arguments","code_sha256":super::sha(creation),"caller":w(deployer),"prestate":[],"execution":execution_json(&e)});
        save(self.calls, &record)?;
        self.cases.push(record);
        ensure!(
            e.exit == Exit::Return(self.runtime.to_vec()),
            "constructor returned exact runtime: {:?}",
            e.exit
        );
        let mut expected = State::new();
        set_role(&mut expected, 0.into(), &[admin]);
        set_role(&mut expected, role("PAUSER_ROLE"), &[admin]);
        for (slot, name) in [(3, b"ALPHEA Point Token".as_slice()), (4, b"APT".as_slice())] {
            let mut packed = [0u8; 32];
            packed[..name.len()].copy_from_slice(name);
            packed[31] = (name.len() * 2) as u8;
            expected.insert(slot.into(), U256::from_big_endian(&packed));
        }
        ensure!(normalize(&e.committed) == normalize(&expected), "constructor exact synthetic storage");
        ensure!(e.logs.len() == 2 && e.committed_logs == e.logs, "constructor role logs");
        for (log, r) in e.logs.iter().zip([U256::zero(), role("PAUSER_ROLE")]) {
            ensure!(
                log.topics == vec![role("RoleGranted(bytes32,address,address)"), r, admin, deployer] && log.data.is_empty(),
                "constructor exact role log"
            );
            let bool_write = e.writes.iter().find(|v| v.key == member_key(r, admin)).context("constructor bool write")?;
            let length_write = e.writes.iter().find(|v| v.key == head(r)).context("constructor length write")?;
            ensure!(bool_write.step < log.step && log.step < length_write.step, "constructor bool/log/set order");
        }
        self.verify_set("constructor_admin", 0.into(), &[admin], 0.into(), &e.committed, save)?;
        self.verify_set("constructor_pauser", role("PAUSER_ROLE"), &[admin], 0.into(), &e.committed, save)?;
        self.getter("constructor_zero_supply", "totalSupply()", &[], 0.into(), &e.committed, save)?;
        Ok(())
    }
    fn sequence_and_failure_controls(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let r = role("SEQUENCE");
        let mut state = base();
        let mut members = vec![];
        for (i, (grant, member)) in [(true, 2u64), (true, 3), (false, 2), (true, 2), (false, 3), (false, 2)].iter().enumerate() {
            let name = format!("sequence_{i}");
            let signature = if *grant {
                "grantRole(bytes32,address)"
            } else {
                "revokeRole(bytes32,address)"
            };
            let member = U256::from(*member);
            let e = self.run(&name, signature, &[r, member], 1.into(), &state, save)?;
            ensure!(e.exit == Exit::Return(vec![]), "sequence");
            state = e.committed;
            if *grant {
                members.push(member);
            } else {
                let index = members.iter().position(|v| *v == member).unwrap();
                members.swap_remove(index);
            }
            self.verify_set(&name, r, &members, member, &state, save)?;
        }
        // Explicitly outside the coherent-state admission assumption. The raw
        // array push wraps a maximum length; this is NOT a coherent set result.
        let mut pre = base();
        pre.insert(head(r), U256::max_value());
        let e = self.run("incoherent_max_length_wrap", "grantRole(bytes32,address)", &[r, 2.into()], 1.into(), &pre, save)?;
        ensure!(
            e.exit == Exit::Return(vec![]) && get(&e.committed, head(r)).is_zero() && get(&e.committed, position(r, 2.into())).is_zero(),
            "source push wraps outside coherent domain"
        );
        ensure!(e.writes.len() == 4 && e.logs.len() == 1, "out-of-domain attempted writes");
        // A different incoherent state reverts after bool SSTORE/RoleRevoked:
        // nonzero position with zero length triggers checked length-1.
        let mut pre = base();
        pre.insert(member_key(r, 2.into()), 1.into());
        pre.insert(position(r, 2.into()), 1.into());
        let e = self.run(
            "incoherent_empty_set_prefix_revert",
            "revokeRole(bytes32,address)",
            &[r, 2.into()],
            1.into(),
            &pre,
            save,
        )?;
        ensure!(
            e.exit == Exit::Revert(call("Panic(uint256)", &[0x11.into()])) && e.committed == pre && e.committed_logs.is_empty(),
            "incoherent-state exact rollback"
        );
        ensure!(
            e.writes.len() == 1 && e.writes[0].key == member_key(r, 2.into()) && e.logs.len() == 1,
            "bool/log prefix preserved"
        );
        // ABI-domain and unknown selector controls cannot be counted as success.
        let mut data = call("grantRole(bytes32,address)", &[r, U256::one() << 160]);
        for (name, calldata) in [
            ("noncanonical_address", data.clone()),
            ("truncated_calldata", {
                data.truncate(20);
                data
            }),
            ("unknown_selector", vec![0xff; 4]),
        ] {
            let e = vm::execute(self.runtime, &calldata, 1.into(), 2.into(), &base());
            self.calls += 1;
            let record =
                json!({"name":name,"caller":w(1.into()),"calldata":hex::encode(calldata),"prestate":state_json(&base()),"execution":execution_json(&e)});
            save(self.calls, &record)?;
            self.cases.push(record);
            ensure!(
                e.exit == Exit::Revert(vec![]) && e.writes.is_empty() && e.logs.is_empty() && e.committed == base(),
                "malformed ABI {name}"
            );
        }
        Ok(())
    }
    fn balance_matrix(&mut self, save: &mut impl FnMut(usize, &Value) -> Result<()>) -> Result<()> {
        let mut state = base();
        set_role(&mut state, role("MINTER_ROLE"), &[1.into()]);
        set_role(&mut state, role("PAUSER_ROLE"), &[1.into()]);
        set_role(&mut state, role("DISTRIBUTOR_ROLE"), &[2.into()]);
        for (name, sig, args, caller, want_supply, want_dist, want_user) in [
            ("mint", "mint(address,uint256)", vec![2.into(), 100.into()], 1, 100, 100, 0),
            ("claim", "transfer(address,uint256)", vec![3.into(), 25.into()], 2, 100, 75, 25),
            ("burn", "burn(uint256)", vec![10.into()], 2, 90, 65, 25),
        ] {
            let e = self.run(name, sig, &args, (caller as u64).into(), &state, save)?;
            ensure!(matches!(e.exit, Exit::Return(_)), "{name} success");
            let mut expected = state.clone();
            expected.insert(2.into(), (want_supply as u64).into());
            expected.insert(mapping(2.into(), 0.into()), (want_dist as u64).into());
            expected.insert(mapping(3.into(), 0.into()), (want_user as u64).into());
            ensure!(
                normalize(&e.committed) == normalize(&expected),
                "{name}: exact all-cell state including role roots"
            );
            let (from, to, amount) = match name {
                "mint" => (0u64, 2u64, 100u64),
                "claim" => (2, 3, 25),
                "burn" => (2, 0, 10),
                _ => unreachable!(),
            };
            ensure!(
                e.logs.len() == 1
                    && e.logs[0].topics == vec![role("Transfer(address,address,uint256)"), from.into(), to.into()]
                    && e.logs[0].data == vm::word(amount.into())
                    && e.committed_logs == e.logs,
                "{name}: exact transfer log"
            );
            state = e.committed;
            for (sig, args, want) in [
                ("totalSupply()", vec![], want_supply),
                ("balanceOf(address)", vec![2.into()], want_dist),
                ("balanceOf(address)", vec![3.into()], want_user),
            ] {
                self.getter(name, sig, &args, (want as u64).into(), &state, save)?;
            }
        }
        let approve = self.run("approve", "approve(address,uint256)", &[4.into(), 20.into()], 2.into(), &state, save)?;
        ensure!(approve.exit == Exit::Return(vm::word(1.into()).to_vec()), "approve");
        let mut expected = state.clone();
        expected.insert(mapping(4.into(), mapping(2.into(), 1.into())), 20.into());
        ensure!(
            normalize(&approve.committed) == normalize(&expected) && approve.writes.len() == 1,
            "approve exact state"
        );
        ensure!(
            approve.logs.len() == 1
                && approve.logs[0].topics == vec![role("Approval(address,address,uint256)"), 2.into(), 4.into()]
                && approve.logs[0].data == vm::word(20.into())
                && approve.committed_logs == approve.logs,
            "approve exact log"
        );
        state = approve.committed;
        let e = self.run(
            "transfer_from_prefix_rollback",
            "transferFrom(address,address,uint256)",
            &[2.into(), 3.into(), 5.into()],
            4.into(),
            &state,
            save,
        )?;
        ensure!(
            e.exit == Exit::Revert(call("RestrictedTransfer(address,address,address)", &[4.into(), 2.into(), 3.into()]))
                && e.committed == state
                && e.committed_logs.is_empty(),
            "transferFrom rollback"
        );
        ensure!(
            e.writes.len() == 1
                && e.writes[0].key == mapping(4.into(), mapping(2.into(), 1.into()))
                && e.writes[0].old == 20.into()
                && e.writes[0].new == 15.into(),
            "executed allowance prefix"
        );
        for (name, sig, args, caller, error) in [
            (
                "mint_invalid_distributor",
                "mint(address,uint256)",
                vec![3.into(), 1.into()],
                1,
                call("InvalidDistributor(address)", &[3.into()]),
            ),
            ("mint_zero", "mint(address,uint256)", vec![2.into(), 0.into()], 1, call("InvalidAmount()", &[])),
            ("burn_zero", "burn(uint256)", vec![0.into()], 2, call("InvalidAmount()", &[])),
            (
                "user_transfer_restricted",
                "transfer(address,uint256)",
                vec![4.into(), 1.into()],
                3,
                call("RestrictedTransfer(address,address,address)", &[3.into(), 3.into(), 4.into()]),
            ),
        ] {
            let e = self.run(name, sig, &args, (caller as u64).into(), &state, save)?;
            ensure!(
                e.exit == Exit::Revert(error) && e.committed == state && e.writes.is_empty() && e.logs.is_empty(),
                "{name}: failure"
            );
        }
        let e = self.run("pause", "pause()", &[], 1.into(), &state, save)?;
        ensure!(e.exit == Exit::Return(vec![]) && get(&e.committed, 7.into()) == 1.into(), "pause");
        let mut expected = state.clone();
        expected.insert(7.into(), 1.into());
        ensure!(
            normalize(&e.committed) == normalize(&expected)
                && e.writes.len() == 1
                && e.logs.len() == 1
                && e.logs[0].topics == vec![role("Paused(address)")]
                && e.logs[0].data == vm::word(1.into())
                && e.committed_logs == e.logs,
            "pause exact state/log"
        );
        state = e.committed;
        let e = self.run("paused_mint", "mint(address,uint256)", &[2.into(), 1.into()], 1.into(), &state, save)?;
        ensure!(e.exit == Exit::Revert(call("EnforcedPause()", &[])) && e.committed == state, "paused mint");
        self.getter("paused_balance", "balanceOf(address)", &[2.into()], 65.into(), &state, save)?;
        self.getter(
            "paused_membership",
            "hasRole(bytes32,address)",
            &[role("DISTRIBUTOR_ROLE"), 2.into()],
            1.into(),
            &state,
            save,
        )?;
        let e = self.run("unpause", "unpause()", &[], 1.into(), &state, save)?;
        ensure!(e.exit == Exit::Return(vec![]) && get(&e.committed, 7.into()).is_zero(), "unpause");
        let mut expected = state.clone();
        expected.insert(7.into(), 0.into());
        ensure!(
            normalize(&e.committed) == normalize(&expected)
                && e.writes.len() == 1
                && e.logs.len() == 1
                && e.logs[0].topics == vec![role("Unpaused(address)")]
                && e.logs[0].data == vm::word(1.into())
                && e.committed_logs == e.logs,
            "unpause exact state/log"
        );
        Ok(())
    }
}
