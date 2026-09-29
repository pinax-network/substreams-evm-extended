//! Independent state/effect expectations for exact runtime and extracted-source semantics.
use super::{vm, CONTRACT};
use crate::ptoken_proof::cases::{call, execution_json, get, mapping, state_json};
use anyhow::{ensure, Result};
use primitive_types::U256;
use serde_json::{json, Value};
use vm::{Exit, State};
pub type Record = [U256; 3];
pub fn head(user: U256) -> U256 {
    mapping(user, 31.into())
}
pub fn element(user: U256, index: U256, field: u64) -> U256 {
    vm::hash(&vm::word(head(user)))
        .overflowing_add(index.overflowing_mul(3.into()).0)
        .0
        .overflowing_add(field.into())
        .0
}
pub fn credit(user: U256) -> U256 {
    mapping(user, 32.into())
}
fn words(v: &[U256]) -> Vec<u8> {
    v.iter().flat_map(|v| vm::word(*v)).collect()
}
fn panic(code: u64) -> Exit {
    Exit::Revert(call("Panic(uint256)", &[code.into()]))
}
fn error(s: &str) -> Exit {
    let mut v = call("Error(string)", &[32.into(), s.len().into()]);
    v.extend(s.as_bytes());
    v.resize(4 + (v.len() - 4).div_ceil(32) * 32, 0);
    Exit::Revert(v)
}
fn normalized(s: &State) -> State {
    s.iter().filter(|(_, v)| !v.is_zero()).map(|(k, v)| (*k, *v)).collect()
}
pub fn seed(user: U256, records: &[Record], available: U256) -> State {
    let mut s = State::new();
    for i in 0..31 {
        s.insert(i.into(), (1000 + i).into());
    }
    s.insert(11.into(), 1.into());
    s.insert(mapping(user, 5.into()), 12345.into());
    s.insert(head(user), records.len().into());
    s.insert(credit(user), available);
    for (i, r) in records.iter().enumerate() {
        for (f, v) in r.iter().enumerate() {
            s.insert(element(user, i.into(), f as u64), *v);
        }
    }
    s.insert(head(999.into()), 1.into());
    s.insert(element(999.into(), 0.into(), 0), U256::MAX);
    s.insert(credit(999.into()), 777.into());
    s.insert(777.into(), 888.into());
    s
}
struct Expected {
    state: State,
    writes: Vec<(U256, U256, U256)>,
    exit: Exit,
}
impl Expected {
    fn new(pre: &State, exit: Exit) -> Self {
        Self {
            state: pre.clone(),
            writes: vec![],
            exit,
        }
    }
    fn store(&mut self, key: U256, value: U256) {
        self.writes.push((key, get(&self.state, key), value));
        self.state.insert(key, value);
    }
}
fn cleanup(pre: &State, user: U256, records: &[Record], now: U256) -> Expected {
    let mut e = Expected::new(pre, Exit::Return(vec![]));
    let k = records.iter().take_while(|r| r[2] <= now).count();
    let expired = records[..k].iter().fold(U256::zero(), |a, r| a.overflowing_add(r[0]).0);
    if expired.is_zero() {
        return e;
    }
    let remaining = records.len() - k;
    for i in 0..remaining {
        for field in 0..3 {
            e.store(element(user, i.into(), field as u64), records[k + i][field]);
        }
    }
    for i in (remaining..records.len()).rev() {
        for field in 0..3 {
            e.store(element(user, i.into(), field), 0.into());
        }
        e.store(head(user), i.into());
    }
    e.store(credit(user), get(pre, credit(user)).overflowing_add(expired).0);
    e
}
fn aggregates(pre: &State, user: U256, records: &[Record], now: U256, details: bool) -> Expected {
    let k = records.iter().take_while(|r| r[2] <= now).count();
    let sum = |rs: &[Record]| -> Option<U256> {
        rs.iter().try_fold(U256::zero(), |a, r| {
            let (v, o) = a.overflowing_add(r[0]);
            (!o).then_some(v)
        })
    };
    let base = get(pre, credit(user));
    let output = if details {
        sum(&records[..k]).and_then(|expired| {
            sum(&records[k..]).and_then(|locked| {
                let (total, overflow) = base.overflowing_add(expired);
                (!overflow).then(|| words(&[base, locked, expired, total, records.len().into()]))
            })
        })
    } else {
        records[..k]
            .iter()
            .try_fold(base, |a, r| {
                let (v, o) = a.overflowing_add(r[0]);
                (!o).then_some(v)
            })
            .map(|v| words(&[v]))
    };
    Expected::new(pre, output.map_or_else(|| panic(0x11), Exit::Return))
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
    fn run(
        &mut self,
        name: &str,
        signature: &str,
        args: &[U256],
        caller: U256,
        now: Option<U256>,
        pre: &State,
        expected: Expected,
        save: &mut impl FnMut(&Value) -> Result<()>,
    ) -> Result<()> {
        let data = call(signature, args);
        self.run_bytes(name, signature, &data, caller, now, pre, expected, save)
    }
    #[allow(clippy::too_many_arguments)]
    fn run_bytes(
        &mut self,
        name: &str,
        signature: &str,
        data: &[u8],
        caller: U256,
        now: Option<U256>,
        pre: &State,
        expected: Expected,
        save: &mut impl FnMut(&Value) -> Result<()>,
    ) -> Result<()> {
        let address = U256::from_big_endian(&hex::decode(&CONTRACT[2..])?);
        let execution = vm::execute_with_timestamp(self.code, data, caller, address, pre, now);
        let record = json!({"name":name,"signature":signature,"calldata":format!("0x{}",hex::encode(data)),"caller":crate::ptoken_proof::cases::w(caller),"timestamp":now.map(crate::ptoken_proof::cases::w),"prestate":state_json(pre),"execution":execution_json(&execution),"expected_state":state_json(&expected.state)});
        save(&record)?; // Preserve raw attempted effects before any assertion can fail.
        ensure!(
            execution.exit == expected.exit,
            "{name}: expected {:?}, got {:?}",
            expected.exit,
            execution.exit
        );
        ensure!(
            normalized(&execution.committed) == normalized(&expected.state),
            "{name}: all-cell state/rollback"
        );
        ensure!(execution.logs.is_empty() && execution.committed_logs.is_empty(), "{name}: unexpected log");
        let actual: Vec<_> = execution.writes.iter().map(|w| (w.key, w.old, w.new)).collect();
        ensure!(
            actual == expected.writes,
            "{name}: exact ordered stores expected {:?}, got {:?}",
            expected.writes,
            actual
        );
        let read = |key| execution.reads.iter().any(|r| r.key == key);
        match signature {
            "createLPInfo(address,uint256)" => {
                ensure!(read(11.into()), "{name}: miner authorization read");
                if matches!(expected.exit, Exit::Return(_)) || name.starts_with("append_length_") {
                    ensure!(read(head(args_user(data)?)), "{name}: length read");
                }
            }
            "process(address)" | "lpInfos(address,uint256)" => ensure!(read(head(args_user(data)?)), "{name}: array head read"),
            "getWithdrawableLPAmount(address)" | "getUserLPDetails(address)" | "lpAmount(address)" => {
                ensure!(read(credit(args_user(data)?)), "{name}: root32 read")
            }
            "balanceOf(address)" => ensure!(read(mapping(args_user(data)?, 5.into())), "{name}: balance read"),
            _ => {}
        }
        for pair in execution.writes.windows(2) {
            ensure!(pair[0].step < pair[1].step, "unique ordered effects");
        }
        self.cases.push(record);
        Ok(())
    }
    pub fn getters(
        &mut self,
        user: U256,
        records: &[Record],
        available: U256,
        now: U256,
        prefix: &str,
        save: &mut impl FnMut(&Value) -> Result<()>,
    ) -> Result<()> {
        let pre = seed(user, records, available);
        for (sig, details) in [("getWithdrawableLPAmount(address)", false), ("getUserLPDetails(address)", true)] {
            self.run(
                &format!("{prefix}:{sig}"),
                sig,
                &[user],
                2.into(),
                Some(now),
                &pre,
                aggregates(&pre, user, records, now, details),
                save,
            )?;
        }
        self.run(
            &format!("{prefix}:credit"),
            "lpAmount(address)",
            &[user],
            2.into(),
            Some(now),
            &pre,
            Expected::new(&pre, Exit::Return(words(&[available]))),
            save,
        )?;
        for i in 0..=records.len() {
            let exit = if i == records.len() {
                Exit::Revert(vec![])
            } else {
                Exit::Return(words(&records[i]))
            };
            self.run(
                &format!("{prefix}:record{i}"),
                "lpInfos(address,uint256)",
                &[user, i.into()],
                2.into(),
                Some(now),
                &pre,
                Expected::new(&pre, exit),
                save,
            )?;
        }
        Ok(())
    }
    pub fn full_matrix(&mut self, save: &mut impl FnMut(&Value) -> Result<()>) -> Result<()> {
        let user = 55.into();
        let limit = U256::MAX - U256::from(8_640_000);
        for (n, now, amount) in [
            (0, 0.into(), 1.into()),
            (1, 10.into(), 9.into()),
            (3, limit, U256::MAX),
            (3, 0.into(), 1.into()),
        ] {
            let rows = vec![[7.into(), 2.into(), 10.into()]; n];
            let pre = seed(user, &rows, 99.into());
            let mut e = Expected::new(&pre, Exit::Return(vec![]));
            e.store(head(user), (n + 1).into());
            for (f, v) in [amount, now, now + 8_640_000].into_iter().enumerate() {
                e.store(element(user, n.into(), f as u64), v);
            }
            self.run(
                &format!("append{n}"),
                "createLPInfo(address,uint256)",
                &[user, amount],
                1.into(),
                Some(now),
                &pre,
                e,
                save,
            )?;
            self.run(
                &format!("balance{n}"),
                "balanceOf(address)",
                &[user],
                2.into(),
                None,
                &pre,
                Expected::new(&pre, Exit::Return(words(&[12345.into()]))),
                save,
            )?;
        }
        let pre = seed(user, &[], 0.into());
        for (name, caller, to, amount, time, exit) in [
            ("wrong_caller", 2.into(), user, 1.into(), U256::MAX, error("Only miner contract can call")),
            ("zero_user", 1.into(), 0.into(), 0.into(), U256::MAX, error("Invalid user address")),
            ("zero_amount", 1.into(), user, 0.into(), U256::MAX, error("Amount must be greater than 0")),
            ("timestamp_overflow", 1.into(), user, 1.into(), limit + 1, panic(0x11)),
        ] {
            self.run(
                name,
                "createLPInfo(address,uint256)",
                &[to, amount],
                caller,
                Some(time),
                &pre,
                Expected::new(&pre, exit),
                save,
            )?;
        }
        // This exact viaIR struct-array push permits old length < 2^64.
        // The MAX-wrap assumption was rejected by the saved actual execution.
        for length in [(U256::one() << 64) - 1, U256::one() << 64, U256::MAX] {
            let mut huge = pre.clone();
            huge.insert(head(user), length);
            let mut e = Expected::new(&huge, if length < (U256::one() << 64) { Exit::Return(vec![]) } else { panic(0x41) });
            if length < (U256::one() << 64) {
                e.store(head(user), length + 1);
                for (f, v) in [1.into(), 10.into(), 8_640_010.into()].into_iter().enumerate() {
                    e.store(element(user, length, f as u64), v);
                }
            }
            self.run(
                &format!("append_length_{length}"),
                "createLPInfo(address,uint256)",
                &[user, 1.into()],
                1.into(),
                Some(10.into()),
                &huge,
                e,
                save,
            )?;
        }
        for data in [vec![0xde, 0xad, 0xbe, 0xef], call("createLPInfo(address,uint256)", &[user])] {
            self.run_bytes(
                "malformed_abi",
                "malformed",
                &data,
                1.into(),
                Some(1.into()),
                &pre,
                Expected::new(&pre, Exit::Revert(vec![])),
                save,
            )?;
        }
        for (i, (rows, credit, now)) in scenarios().into_iter().enumerate() {
            self.getters(user, &rows, credit, now, &format!("full{i}"), save)?;
        }
        Ok(())
    }
    pub fn harness_matrix(&mut self, save: &mut impl FnMut(&Value) -> Result<()>) -> Result<()> {
        let user = 55.into();
        for (i, (rows, credit, now)) in scenarios().into_iter().enumerate() {
            let pre = seed(user, &rows, credit);
            let expected = cleanup(&pre, user, &rows, now);
            self.run(&format!("cleanup{i}"), "process(address)", &[user], 2.into(), Some(now), &pre, expected, save)?;
            self.getters(user, &rows, credit, now, &format!("harness{i}"), save)?;
        }
        Ok(())
    }
    pub fn boundary(&mut self, name: &str, data: &[u8], pre: &State, expected_reason: &str, save: &mut impl FnMut(&Value) -> Result<()>) -> Result<()> {
        let address = U256::from_big_endian(&hex::decode(&CONTRACT[2..])?);
        let e = vm::execute_with_timestamp(self.code, data, 1.into(), address, pre, Some(10.into()));
        let record = json!({"name":name,"signature":"unsupported_boundary","calldata":format!("0x{}",hex::encode(data)),"prestate":state_json(pre),"execution":execution_json(&e),"expected_boundary":expected_reason});
        save(&record)?;
        ensure!(
            matches!(e.exit,Exit::HarnessFailure(ref why) if why.contains(expected_reason)),
            "exact {name} unsupported boundary: {:?}",
            e.exit
        );
        ensure!(e.committed == *pre && e.committed_logs.is_empty(), "unsupported rollback");
        if name == "original_constructor_unsupported" {
            let word = |h: &str| U256::from_str_radix(h, 16).unwrap();
            let address = U256::from_big_endian(&hex::decode(&CONTRACT[2..])?);
            let router = word("10ed43c718714eb63d5aa57b78b54704e256024e");
            let fees = [150u64, 150, 100, 100, 150, 150, 100, 200]
                .into_iter()
                .enumerate()
                .fold(U256::zero(), |a, (i, v)| a | (U256::from(v) << (16 * i)));
            let unit = U256::from(10).pow(18.into());
            let packed_name = U256::from_big_endian(&[b"TOPS".as_slice(), &[0u8; 28]].concat()) | U256::from(8);
            let stores = [
                (word("9b779b17422d0df92223018b32b4d1fa46e071723d6817e2486d003becc55f00"), 1.into()),
                (0.into(), U256::one() | (U256::from(500000) << 160)),
                (9.into(), U256::one() << 16),
                (10.into(), 100.into()),
                (20.into(), fees),
                (21.into(), (U256::from(300) * unit) | (U256::from(100_000_000_000_000u64) << 128)),
                (22.into(), 0.into()),
                (23.into(), (U256::from(1000) * unit) | (U256::from(100_000_000_000_000u64) << 128)),
                (24.into(), 0.into()),
                (25.into(), U256::from(300) * unit),
                (33.into(), 0.into()),
                (34.into(), U256::from(28800) << 32),
                (1.into(), packed_name),
                (2.into(), packed_name),
                (3.into(), 18.into()),
                (mapping(router, 8.into()), 1.into()),
                (mapping(router, mapping(address, 6.into())), U256::MAX),
            ];
            ensure!(
                e.writes
                    .iter()
                    .map(|w| (w.key, w.old, w.new))
                    .eq(stores.into_iter().map(|(k, v)| (k, U256::zero(), v))),
                "exact independently packed constructor prefix"
            );
            ensure!(
                e.logs.len() == 1 && e.logs[0].topics == [vm::hash(b"OwnershipTransferred(address,address)"), 0.into(), 1.into()] && e.logs[0].data.is_empty(),
                "exact attempted ownership log"
            );
            ensure!(e.logs[0].step < e.writes[0].step, "ownership log interleaving");
        } else {
            ensure!(e.writes.is_empty() && e.logs.is_empty(), "transfer boundary before all effects");
        }
        self.cases.push(record);
        Ok(())
    }
}
fn args_user(data: &[u8]) -> Result<U256> {
    ensure!(data.len() >= 36, "address calldata");
    Ok(U256::from_big_endian(&data[4..36]))
}
pub fn scenarios() -> Vec<(Vec<Record>, U256, U256)> {
    let mut cases = vec![];
    for n in 0..=6 {
        for cutoff in 0..=n {
            let rows = (0..n)
                .map(|i| [(i + 1).into(), (i + 20).into(), if i < cutoff { 10.into() } else { 11.into() }])
                .collect();
            cases.push((rows, 37.into(), 10.into()));
        }
    }
    cases.extend([
        (vec![[1.into(), 0.into(), 0.into()], [U256::MAX, 0.into(), 0.into()]], 99.into(), 0.into()),
        (vec![[2.into(), 0.into(), 0.into()], [U256::MAX, 0.into(), 0.into()]], U256::MAX, 0.into()),
        (vec![[0.into(), 0.into(), 0.into()]], 7.into(), 0.into()),
        (vec![[9.into(), 0.into(), 11.into()], [8.into(), 0.into(), 0.into()]], 0.into(), 10.into()),
        (vec![[7.into(), 2.into(), 10.into()]; 5], U256::MAX, 10.into()),
        (
            vec![
                [0.into(), 0.into(), 10.into()],
                [4.into(), 0.into(), 10.into()],
                [4.into(), 0.into(), 11.into()],
                [4.into(), 0.into(), 11.into()],
            ],
            0.into(),
            10.into(),
        ),
        (vec![[1.into(), U256::MAX, U256::MAX]], U256::MAX, U256::MAX),
        (vec![[U256::MAX, 0.into(), 11.into()], [1.into(), 0.into(), 11.into()]], 0.into(), 10.into()),
    ]);
    cases
}
