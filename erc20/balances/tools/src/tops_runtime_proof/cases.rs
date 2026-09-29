//! Independent whole-state and ordered-effect expectations for the original runtime.
use super::{vm, CONTRACT};
use crate::ptoken_proof::cases::{call, execution_json, get, mapping, state_json, w};
use crate::tops_proof::cases::{credit, element, head, Record};
use anyhow::{ensure, Context, Result};
use primitive_types::U256;
use serde_json::{json, Value};
use vm::{ContextExecution, Exit, GasExpectation, ReadOnlyContext, ScriptedResult, State, StaticCallExpectation};
pub const PAIR: &str = "8ec426ffb466990ac36048ef5c90348783ea4272";
pub const USDT: &str = "55d398326f99059ff775485246999027b3197955";
pub fn address(s: &str) -> U256 {
    U256::from_str_radix(s.trim_start_matches("0x"), 16).unwrap()
}
fn words(v: &[U256]) -> Vec<u8> {
    v.iter().flat_map(|v| vm::word(*v)).collect()
}
fn normalize(s: &State) -> State {
    s.iter().filter(|(_, v)| !v.is_zero()).map(|(k, v)| (*k, *v)).collect()
}
pub fn balance(user: U256) -> U256 {
    mapping(user, 5.into())
}
pub fn allowance(owner: U256, spender: U256) -> U256 {
    mapping(spender, mapping(owner, 6.into()))
}
pub fn script(origin: U256, now: U256) -> ReadOnlyContext {
    let gas = U256::from(1_000_000);
    ReadOnlyContext {
        origin,
        timestamp: Some(now),
        gas_reads: vec![GasExpectation { pc: 7757, value: gas }, GasExpectation { pc: 8142, value: gas }],
        calls: vec![
            StaticCallExpectation {
                pc: 7758,
                address: address(PAIR),
                gas,
                input: hex::decode("0902f1ac").unwrap(),
                output_size: 96,
                result: ScriptedResult::Return(words(&[0.into(), 0.into(), 0.into()])),
            },
            StaticCallExpectation {
                pc: 8143,
                address: address(USDT),
                gas,
                input: call("balanceOf(address)", &[address(PAIR)]),
                output_size: 32,
                result: ScriptedResult::Return(vm::word(0.into()).to_vec()),
            },
        ],
    }
}
fn empty(origin: U256, now: U256) -> ReadOnlyContext {
    ReadOnlyContext {
        origin,
        timestamp: Some(now),
        gas_reads: vec![],
        calls: vec![],
    }
}
pub fn seed(owner: U256, records: &[Record], available: U256, from: U256, to: U256) -> State {
    let mut s = State::new();
    for i in 0..35 {
        s.insert(i.into(), (2000 + i).into());
    }
    s.insert(9.into(), 0.into());
    s.insert(balance(from), 1000.into());
    if to != from {
        s.insert(balance(to), 77.into());
    }
    s.insert(head(owner), records.len().into());
    s.insert(credit(owner), available);
    for (i, r) in records.iter().enumerate() {
        for (f, v) in r.iter().enumerate() {
            s.insert(element(owner, i.into(), f as u64), *v);
        }
    }
    // Explicit untouched unrelated account and post-length sentinel cells.
    s.insert(head(999.into()), 1.into());
    s.insert(element(999.into(), 0.into(), 0), U256::MAX);
    s.insert(credit(999.into()), 888.into());
    for f in 0..3 {
        s.insert(element(owner, (records.len() + 2).into(), f), (777 + f).into());
    }
    s
}
#[derive(Clone)]
pub struct Expected {
    pub state: State,
    pub writes: Vec<(U256, U256, U256)>,
    pub logs: Vec<(Vec<U256>, Vec<u8>)>,
    pub log_after_stores: Vec<usize>,
    pub exit: Exit,
}
impl Expected {
    fn new(pre: &State, exit: Exit) -> Self {
        Self {
            state: pre.clone(),
            writes: vec![],
            logs: vec![],
            log_after_stores: vec![],
            exit,
        }
    }
    fn store(&mut self, key: U256, value: U256) {
        self.writes.push((key, get(&self.state, key), value));
        self.state.insert(key, value);
    }
}
fn panic(code: u64) -> Exit {
    Exit::Revert(call("Panic(uint256)", &[code.into()]))
}
fn error(text: &str) -> Exit {
    let mut b = call("Error(string)", &[32.into(), text.len().into()]);
    b.extend(text.as_bytes());
    b.resize(4 + (b.len() - 4).div_ceil(32) * 32, 0);
    Exit::Revert(b)
}
/// Independent prefix scan, forward copy, tail clearing and wrapped credit model.
#[allow(clippy::too_many_arguments)]
pub fn expected_transfer(pre: &State, owner: U256, records: &[Record], now: U256, from: U256, to: U256, amount: U256, spender: Option<U256>) -> Expected {
    let mut e = Expected::new(pre, Exit::Return(vm::word(1.into()).to_vec()));
    if get(pre, balance(from)) < amount {
        e.exit = error("BNE");
        return e;
    }
    let expired_count = records.iter().take_while(|r| r[2] <= now).count();
    let expired = records[..expired_count].iter().fold(U256::zero(), |x, r| x.overflowing_add(r[0]).0);
    if !expired.is_zero() {
        let remaining = records.len() - expired_count;
        for i in 0..remaining {
            for f in 0..3 {
                e.store(element(owner, i.into(), f), records[expired_count + i][f as usize]);
            }
        }
        for i in (remaining..records.len()).rev() {
            for f in 0..3 {
                e.store(element(owner, i.into(), f), 0.into());
            }
            e.store(head(owner), i.into());
        }
        e.store(credit(owner), get(pre, credit(owner)).overflowing_add(expired).0);
    }
    e.store(balance(from), get(&e.state, balance(from)) - amount);
    e.store(balance(to), get(&e.state, balance(to)).overflowing_add(amount).0);
    e.log_after_stores.push(e.writes.len());
    e.logs
        .push((vec![vm::hash(b"Transfer(address,address,uint256)"), from, to], vm::word(amount).to_vec()));
    if let Some(spender) = spender {
        let key = allowance(from, spender);
        let old = get(pre, key);
        if old != U256::MAX {
            if old < amount {
                e.exit = panic(0x11);
            } else {
                e.store(key, old - amount);
            }
        }
    }
    e
}
fn result_json(r: &ScriptedResult) -> Value {
    match r {
        ScriptedResult::Return(b) => json!({"kind":"return","data":hex::encode(b)}),
        ScriptedResult::Revert(b) => json!({"kind":"revert","data":hex::encode(b)}),
    }
}
pub fn context_json(c: &ReadOnlyContext) -> Value {
    json!({"origin":w(c.origin),"timestamp":c.timestamp.map(w),"gas_reads":c.gas_reads.iter().map(|x|json!({"pc":x.pc,"value":w(x.value)})).collect::<Vec<_>>(),"calls":c.calls.iter().map(|x|json!({"pc":x.pc,"address":w(x.address),"gas":w(x.gas),"input":hex::encode(&x.input),"output_size":x.output_size,"result":result_json(&x.result)})).collect::<Vec<_>>()})
}
pub fn witness_json(e: &ContextExecution) -> Value {
    json!({"consumed_calls":e.consumed_calls,"consumed_gas":e.consumed_gas,"origin_reads":e.origin_reads.iter().map(|x|json!({"step":x.step,"pc":x.pc,"value":w(x.value)})).collect::<Vec<_>>(),"gas_reads":e.gas_reads.iter().map(|x|json!({"step":x.step,"pc":x.pc,"value":w(x.value)})).collect::<Vec<_>>(),"calls":e.calls.iter().map(|x|json!({"step":x.step,"pc":x.pc,"raw_address":w(x.raw_address),"address":w(x.address),"gas":w(x.gas),"input_offset":w(x.input_offset),"input_size":w(x.input_size),"output_offset":w(x.output_offset),"output_size":w(x.output_size),"input":hex::encode(&x.input),"result":x.result.as_ref().map(result_json)})).collect::<Vec<_>>()})
}
pub struct Proof<'a> {
    pub code: &'a [u8],
    pub cases: Vec<Value>,
}
impl<'a> Proof<'a> {
    pub fn new(code: &'a [u8]) -> Self {
        Self { code, cases: vec![] }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &mut self,
        name: &str,
        signature: &str,
        data: &[u8],
        caller: U256,
        pre: &State,
        context: &ReadOnlyContext,
        expected: Expected,
        save: &mut impl FnMut(&Value) -> Result<()>,
    ) -> Result<ContextExecution> {
        let e = vm::execute_with_read_only_context(self.code, data, caller, address(CONTRACT), pre, context);
        let record = json!({"name":name,"signature":signature,"code_sha256":super::sha(self.code),"caller":w(caller),"calldata":hex::encode(data),"prestate":state_json(pre),"context":context_json(context),"context_witness":witness_json(&e),"execution":execution_json(&e.execution)});
        save(&record)?;
        self.cases.push(record);
        validate(name, pre, &e, &expected)?;
        Ok(e)
    }
    #[allow(clippy::too_many_arguments)]
    fn transfer(
        &mut self,
        name: &str,
        owner: U256,
        records: &[Record],
        now: U256,
        pre: &State,
        from: U256,
        to: U256,
        amount: U256,
        spender: Option<U256>,
        save: &mut impl FnMut(&Value) -> Result<()>,
    ) -> Result<State> {
        let expected = expected_transfer(pre, owner, records, now, from, to, amount, spender);
        let (signature, args) = if spender.is_some() {
            ("transferFrom(address,address,uint256)", vec![from, to, amount])
        } else {
            ("transfer(address,uint256)", vec![to, amount])
        };
        let c = if get(pre, balance(from)) < amount {
            empty(owner, now)
        } else {
            script(owner, now)
        };
        let e = self.run(name, signature, &call(signature, &args), spender.unwrap_or(from), pre, &c, expected, save)?;
        if matches!(e.execution.exit, Exit::Return(_)) {
            ensure!(e.origin_reads.iter().any(|x| x.value == owner), "{name}: actual ORIGIN read");
            ensure!(e.consumed_calls == 2 && e.consumed_gas == 2, "{name}: exactly two external reads");
        }
        Ok(e.execution.committed)
    }
    #[allow(clippy::too_many_arguments)]
    fn getter(
        &mut self,
        name: &str,
        signature: &str,
        args: &[U256],
        pre: &State,
        now: U256,
        exit: Exit,
        save: &mut impl FnMut(&Value) -> Result<()>,
    ) -> Result<()> {
        self.run(
            name,
            signature,
            &call(signature, args),
            11.into(),
            pre,
            &empty(55.into(), now),
            Expected::new(pre, exit),
            save,
        )?;
        Ok(())
    }
    pub fn matrix(&mut self, save: &mut impl FnMut(&Value) -> Result<()>) -> Result<()> {
        let owner = U256::from(55);
        let from = U256::from(11);
        let to = U256::from(22);
        let now = U256::from(100);
        for n in 0..=6 {
            for k in 0..=n {
                for mode in 0..3 {
                    let records: Vec<Record> = (0..n)
                        .map(|i| {
                            [
                                (if mode == 0 { 0 } else { i as u64 + 1 }).into(),
                                (50 + i).into(),
                                if i < k {
                                    now
                                } else if mode == 2 && i > k {
                                    0.into()
                                } else {
                                    now + 1
                                },
                            ]
                        })
                        .collect();
                    let available = if mode == 2 { U256::MAX } else { 9.into() };
                    let pre = seed(owner, &records, available, from, to);
                    let state = self.transfer(
                        &format!("prefix_n{n}_k{k}_mode{mode}"),
                        owner,
                        &records,
                        now,
                        &pre,
                        from,
                        to,
                        7.into(),
                        None,
                        save,
                    )?;
                    for holder in [from, to] {
                        self.getter(
                            "post_transfer_balance",
                            "balanceOf(address)",
                            &[holder],
                            &state,
                            now,
                            Exit::Return(vm::word(get(&state, balance(holder))).to_vec()),
                            save,
                        )?;
                    }
                    self.getter(
                        "post_cleanup_credit",
                        "lpAmount(address)",
                        &[owner],
                        &state,
                        now,
                        Exit::Return(vm::word(get(&state, credit(owner))).to_vec()),
                        save,
                    )?;
                }
            }
        }
        for (name, amounts) in [
            ("wrapped_expired_zero", vec![1.into(), U256::MAX]),
            ("wrapped_expired_nonzero", vec![2.into(), U256::MAX]),
            ("all_zero_no_cleanup", vec![0.into(), 0.into()]),
        ] {
            let records: Vec<Record> = amounts.into_iter().map(|a| [a, 0.into(), now]).collect();
            let pre = seed(owner, &records, U256::MAX, from, to);
            self.transfer(name, owner, &records, now, &pre, from, to, 0.into(), None, save)?;
            self.getter(
                "checked_withdrawable_before_cleanup",
                "getWithdrawableLPAmount(address)",
                &[owner],
                &pre,
                now,
                if name == "all_zero_no_cleanup" {
                    Exit::Return(vm::word(U256::MAX).to_vec())
                } else {
                    panic(0x11)
                },
                save,
            )?;
            self.getter(
                "checked_details_before_cleanup",
                "getUserLPDetails(address)",
                &[owner],
                &pre,
                now,
                if name == "all_zero_no_cleanup" {
                    Exit::Return(words(&[U256::MAX, 0.into(), 0.into(), U256::MAX, 2.into()]))
                } else {
                    panic(0x11)
                },
                save,
            )?;
        }
        let records = vec![[9.into(), 0.into(), now], [8.into(), 0.into(), now + 1], [7.into(), 0.into(), 0.into()]];
        for (label, timestamp) in [
            ("before_expiry", now - 1),
            ("at_expiry", now),
            ("after_all_expiry", now + 1),
            ("maximum_timestamp", U256::MAX),
        ] {
            let pre = seed(owner, &records, 3.into(), from, to);
            self.transfer(label, owner, &records, timestamp, &pre, from, to, 1.into(), None, save)?;
        }
        for (label, recipient, amount, recipient_balance) in [
            ("zero_transfer", to, 0.into(), 77.into()),
            ("self_transfer", from, 7.into(), 1000.into()),
            ("zero_recipient", 0.into(), 7.into(), 0.into()),
            ("recipient_wrap", to, 7.into(), U256::MAX),
        ] {
            let mut pre = seed(owner, &records, 3.into(), from, recipient);
            pre.insert(balance(recipient), recipient_balance);
            self.transfer(label, owner, &records, now, &pre, from, recipient, amount, None, save)?;
        }
        for (label, origin, in_swap) in [("origin_is_sender_in_swap", from, true), ("distinct_origin_without_swap", owner, false)] {
            let mut pre = seed(origin, &records, 3.into(), from, to);
            pre.insert(9.into(), u8::from(in_swap).into());
            self.transfer(label, origin, &records, now, &pre, from, to, 7.into(), None, save)?;
        }
        for allowed in [0.into(), 6.into(), 7.into(), 8.into(), U256::MAX] {
            for amount in [0.into(), 7.into()] {
                let mut pre = seed(owner, &records, 3.into(), from, to);
                pre.insert(allowance(from, 77.into()), allowed);
                let state = self.transfer(
                    &format!("late_allowance_{allowed}_{amount}"),
                    owner,
                    &records,
                    now,
                    &pre,
                    from,
                    to,
                    amount,
                    Some(77.into()),
                    save,
                )?;
                self.getter(
                    "post_transferFrom_allowance",
                    "allowance(address,address)",
                    &[from, 77.into()],
                    &state,
                    now,
                    Exit::Return(vm::word(get(&state, allowance(from, 77.into()))).to_vec()),
                    save,
                )?;
            }
        }
        let pre = seed(owner, &records, 3.into(), from, to);
        self.transfer(
            "insufficient_balance_before_external",
            owner,
            &records,
            now,
            &pre,
            from,
            to,
            1001.into(),
            None,
            save,
        )?;
        for add in [true, false] {
            let mut branch = pre.clone();
            branch.insert(mapping(if add { to } else { from }, 18.into()), 1.into());
            if add {
                branch.insert(mapping(from, 8.into()), 1.into());
            }
            let mut expected = expected_transfer(&branch, owner, &records, now, from, to, 7.into(), None);
            expected.writes.truncate(expected.writes.len() - 2);
            expected.logs.clear();
            expected.log_after_stores.clear();
            expected.exit = Exit::HarnessFailure("unexpected GAS read".into());
            self.run(
                if add {
                    "add_liquidity_after_cleanup_unsupported"
                } else {
                    "remove_liquidity_after_cleanup_unsupported"
                },
                "explicit_unsupported_boundary",
                &call("transfer(address,uint256)", &[to, 7.into()]),
                from,
                &branch,
                &script(owner, now),
                expected,
                save,
            )?;
        }
        self.external_boundaries(&pre, from, to, save)?;
        self.saved_case(save)?;
        Ok(())
    }
    fn external_boundaries(&mut self, pre: &State, from: U256, to: U256, save: &mut impl FnMut(&Value) -> Result<()>) -> Result<()> {
        let data = call("transfer(address,uint256)", &[to, 7.into()]);
        for target in 0..2 {
            for payload in [vec![], vec![1, 2, 3, 4, 5]] {
                let mut c = script(55.into(), 100.into());
                c.calls.truncate(target + 1);
                c.gas_reads.truncate(target + 1);
                c.calls[target].result = ScriptedResult::Revert(payload.clone());
                self.run(
                    &format!("external_revert_{target}_{}", payload.len()),
                    "transfer(address,uint256)",
                    &data,
                    from,
                    pre,
                    &c,
                    Expected::new(pre, Exit::Revert(payload)),
                    save,
                )?;
            }
        }
        for (target, lengths) in [(0, vec![0, 31, 64, 95]), (1, vec![0, 1, 31])] {
            for len in lengths {
                let mut c = script(55.into(), 100.into());
                c.calls[target].result = ScriptedResult::Return(vec![0; len]);
                if target == 0 {
                    c.calls.truncate(1);
                    c.gas_reads.truncate(1);
                }
                self.run(
                    &format!("malformed_return_{target}_{len}"),
                    "transfer(address,uint256)",
                    &data,
                    from,
                    pre,
                    &c,
                    Expected::new(pre, Exit::Revert(vec![])),
                    save,
                )?;
            }
        }
        // ABI dirty reserve words: source decoder width checks must be independently measured.
        for (field, bits) in [(0, 112), (1, 112), (2, 32)] {
            let mut c = script(55.into(), 100.into());
            let mut fields = [0.into(); 3];
            fields[field] = U256::one() << bits;
            c.calls[0].result = ScriptedResult::Return(words(&fields));
            c.calls.truncate(1);
            c.gas_reads.truncate(1);
            self.run(
                &format!("dirty_reserve_field_{field}"),
                "transfer(address,uint256)",
                &data,
                from,
                pre,
                &c,
                Expected::new(pre, Exit::Revert(vec![])),
                save,
            )?;
        }
        for (name, change) in [
            ("unexpected_pair", 0),
            ("unexpected_selector", 1),
            ("missing_call", 2),
            ("missing_gas", 3),
            ("unexpected_output_width", 4),
        ] {
            let mut c = script(55.into(), 100.into());
            match change {
                0 => c.calls[0].address = 9.into(),
                1 => c.calls[0].input[0] ^= 1,
                2 => c.calls.clear(),
                3 => c.gas_reads.clear(),
                _ => c.calls[0].output_size = 32,
            };
            self.unsupported(name, &data, from, pre, &c, save)?;
        }
        // Nonzero reserves and origin==sender each reach a third router interaction.
        let mut c = script(55.into(), 100.into());
        c.calls[0].result = ScriptedResult::Return(words(&[1.into(), 1.into(), 0.into()]));
        self.unsupported("nonzero_reserves_router_boundary", &data, from, pre, &c, save)?;
        let c = script(from, 100.into());
        self.unsupported("origin_sender_router_catch_cannot_hide_boundary", &data, from, pre, &c, save)?;
        Ok(())
    }
    fn unsupported(
        &mut self,
        name: &str,
        data: &[u8],
        caller: U256,
        pre: &State,
        c: &ReadOnlyContext,
        save: &mut impl FnMut(&Value) -> Result<()>,
    ) -> Result<()> {
        let e = vm::execute_with_read_only_context(self.code, data, caller, address(CONTRACT), pre, c);
        let record = json!({"name":name,"signature":"explicit_unsupported_boundary","code_sha256":super::sha(self.code),"caller":w(caller),"calldata":hex::encode(data),"prestate":state_json(pre),"context":context_json(c),"context_witness":witness_json(&e),"execution":execution_json(&e.execution)});
        save(&record)?;
        self.cases.push(record);
        ensure!(
            matches!(e.execution.exit, Exit::HarnessFailure(_)),
            "{name}: explicit unsupported interaction {:?}",
            e.execution.exit
        );
        ensure!(
            normalize(&e.execution.committed) == normalize(pre) && e.execution.committed_logs.is_empty(),
            "unsupported rollback"
        );
        ensure!(e.execution.writes.is_empty() && e.execution.logs.is_empty(), "no hidden pre-boundary effects");
        Ok(())
    }
    fn saved_case(&mut self, save: &mut impl FnMut(&Value) -> Result<()>) -> Result<()> {
        let v: Value = serde_json::from_slice(super::SAVED_CASES)?;
        let case = v
            .as_array()
            .context("historical cases")?
            .iter()
            .find(|x| x["token"] == "tops")
            .context("TOPS saved case")?;
        ensure!(case["block"] == 123561227 && case["transaction_index"] == 63, "saved interval/transaction");
        let c = &case["calls"][0];
        ensure!(c["index"] == 14, "saved call");
        let from = address(c["caller"].as_str().context("caller")?);
        let owner = address("28ebbb1c3003a30f9e627647862dd85aa4c165ce");
        let data = super::bytes(&c["input"])?;
        let amount = U256::from_big_endian(&data[36..68]);
        ensure!(amount == U256::from_dec_str("393140350383254688654")?, "saved transfer amount");
        let writes: Vec<_> = c["writes"]
            .as_array()
            .context("saved writes")?
            .iter()
            .map(|v| {
                Ok((
                    address(v["key"].as_str().context("key")?),
                    address(v["old"].as_str().context("old")?),
                    address(v["new"].as_str().context("new")?),
                ))
            })
            .collect::<Result<_>>()?;
        ensure!(writes.len() == 23, "saved 23 writes");
        let mut original = State::new();
        for (key, old, _) in &writes {
            original.entry(*key).or_insert(*old);
        }
        ensure!(get(&original, head(owner)) == 5.into(), "five saved LP records");
        let records: Vec<Record> = (0..5)
            .map(|i| {
                [
                    get(&original, element(owner, i.into(), 0)),
                    get(&original, element(owner, i.into(), 1)),
                    get(&original, element(owner, i.into(), 2)),
                ]
            })
            .collect();
        let now = records.iter().map(|r| r[2]).max().context("saved expiries")?;
        let available = get(&original, credit(owner));
        let mut pre = seed(owner, &records, available, from, owner);
        pre.extend(original);
        let expected = expected_transfer(&pre, owner, &records, now, from, owner, amount, None);
        ensure!(expected.writes == writes, "independent model exactly matches all 23 captured writes");
        ensure!(
            records.iter().fold(U256::zero(), |a, r| a + r[0]) == U256::from_dec_str("14594824580393536426")?,
            "saved expired aggregate"
        );
        let mut linked = |v: &Value| {
            let mut v = v.clone();
            v["historical_reference"] = json!({"block":123561227,"transaction_index":63,"call_index":14,"scope":"Only 23 captured store words/order and calldata are compared; origin, time, external responses and unrelated storage are synthetic, not a historical full transaction replay","synthetic_timestamp":"maximum captured expiry","synthetic_origin":"recipient LP owner"});
            save(&v)
        };
        self.run(
            "saved_five_pops_all_23_original_runtime_stores",
            "transfer(address,uint256)",
            &data,
            from,
            &pre,
            &script(owner, now),
            expected,
            &mut linked,
        )?;
        Ok(())
    }
}
pub fn validate(name: &str, pre: &State, e: &ContextExecution, expected: &Expected) -> Result<()> {
    ensure!(
        e.execution.exit == expected.exit,
        "{name}: exit {:?} expected {:?}",
        e.execution.exit,
        expected.exit
    );
    let actual: Vec<_> = e.execution.writes.iter().map(|x| (x.key, x.old, x.new)).collect();
    ensure!(
        actual == expected.writes,
        "{name}: exact ordered stores: actual {actual:?}, expected {:?}",
        expected.writes
    );
    let logs: Vec<_> = e.execution.logs.iter().map(|x| (x.topics.clone(), x.data.clone())).collect();
    ensure!(logs == expected.logs, "{name}: exact attempted logs");
    let success = matches!(expected.exit, Exit::Return(_));
    ensure!(
        normalize(&e.execution.committed) == normalize(if success { &expected.state } else { pre }),
        "{name}: every storage cell/rollback"
    );
    ensure!(
        e.execution.committed_logs.len() == if success { expected.logs.len() } else { 0 },
        "{name}: committed logs/rollback"
    );
    ensure!(
        expected.log_after_stores.len() == e.execution.logs.len(),
        "{name}: log/store expectation cardinality"
    );
    for (log, expected_before) in e.execution.logs.iter().zip(&expected.log_after_stores) {
        let before = e.execution.writes.iter().filter(|x| x.step < log.step).count();
        ensure!(before == *expected_before, "{name}: exact source store/log interleaving");
    }
    Ok(())
}
