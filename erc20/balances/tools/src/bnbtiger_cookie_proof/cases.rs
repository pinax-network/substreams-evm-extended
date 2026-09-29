//! Getter-only synthetic states, paired unpatched compiler/captured programs.
use super::*;
use crate::ptoken_proof::cases::{call, execution_json, mapping, state_json, w};
use vm::{Execution, Exit, State};
#[derive(Default)]
pub struct Counts {
    pub calls: usize,
    pub returns: usize,
    pub reverts: usize,
    pub pairs: usize,
    pub categories: BTreeMap<String, usize>,
}
impl Counts {
    pub fn json(&self) -> Value {
        json!({"calls":self.calls,"returns":self.returns,"reverts":self.reverts,"pairs":self.pairs,"categories":self.categories})
    }
}
/// A successful raw getter is exactly one mapping read, never metadata/caller state.
pub fn verify_getter(e: &Execution, t: Target, holder: U256, value: U256, pre: &State) -> Result<()> {
    ensure!(e.exit == Exit::Return(vm::word(value).to_vec()), "exact getter output");
    let key = mapping(holder, t.root());
    ensure!(
        e.reads.len() == 1 && e.reads[0].key == key && e.reads[0].value == value,
        "one exact balance SLOAD"
    );
    ensure!(
        e.keccaks.len() == 1 && e.keccaks[0].input == [vm::word(holder), vm::word(t.root())].concat() && e.keccaks[0].output == key,
        "one exact address/root preimage"
    );
    ensure!(
        e.writes.is_empty() && e.logs.is_empty() && e.committed_logs.is_empty() && e.committed == *pre,
        "no attempted/committed mutation"
    );
    ensure!(
        e.trace
            .iter()
            .all(|s| !matches!(s.opcode,0x31|0x32|0x33|0x3b|0x3c|0x3f|0x40..=0x4a|0x55|0xa0..=0xa4|0xf0 | 0xf1 | 0xf2 | 0xf4 | 0xf5 | 0xfa | 0xff)),
        "getter has no caller/context/external/effect opcodes"
    );
    Ok(())
}
fn unchanged(e: &Execution, pre: &State) -> Result<()> {
    ensure!(
        e.writes.is_empty() && e.logs.is_empty() && e.committed_logs.is_empty() && e.committed == *pre,
        "ABI probe cannot mutate"
    );
    Ok(())
}
struct Proof<'a, F: FnMut(&Value) -> Result<()>> {
    t: Target,
    codes: [(&'static str, Vec<u8>); 2],
    save: &'a mut F,
    counts: Counts,
}
impl<F: FnMut(&Value) -> Result<()>> Proof<'_, F> {
    fn pair(&mut self, name: &str, category: &str, data: Vec<u8>, caller: U256, pre: State, expected: Expected) -> Result<()> {
        let mut previous = None;
        for (program, code) in &self.codes {
            let e = vm::execute(code, &data, caller, self.t.account(), &pre);
            let execution = execution_json(&e);
            let record = json!({"target":self.t.label(),"program":program,"case":name,"category":category,"runtime_keccak256":kh(code),"address":w(self.t.account()),"caller":w(caller),"calldata":hex::encode(&data),"prestate":state_json(&pre),"expected":expected.json(),"execution":execution});
            (self.save)(&record)?; // Preserve unsuccessful attempts before checking any outcome.
            match expected {
                Expected::Getter(holder, value) => verify_getter(&e, self.t, holder, value, &pre)?,
                Expected::EmptyReturn => {
                    ensure!(e.exit == Exit::Return(vec![]), "exact empty receive return");
                    unchanged(&e, &pre)?;
                }
                Expected::EmptyRevert => {
                    ensure!(e.exit == Exit::Revert(vec![]), "exact ABI/selector empty REVERT, never INVALID/HarnessFailure");
                    ensure!(e.reads.is_empty() && e.keccaks.is_empty(), "ABI failure before storage");
                    unchanged(&e, &pre)?;
                }
            }
            verify_unmodified_path(&e, self.t)?;
            if let Some(before) = previous {
                ensure!(before == execution, "entire paired trace/effects/outcome equality");
            }
            previous = Some(execution);
            self.counts.calls += 1;
            match e.exit {
                Exit::Return(_) => self.counts.returns += 1,
                Exit::Revert(_) => self.counts.reverts += 1,
                _ => anyhow::bail!("unaccepted exit"),
            };
            *self.counts.categories.entry(category.into()).or_default() += 1;
        }
        self.counts.pairs += 1;
        Ok(())
    }
}
#[derive(Clone, Copy)]
enum Expected {
    Getter(U256, U256),
    EmptyReturn,
    EmptyRevert,
}
impl Expected {
    fn json(self) -> Value {
        match self {
            Self::Getter(h, v) => json!({"kind":"getter","holder":w(h),"value":w(v)}),
            Self::EmptyReturn => json!({"kind":"return","data":""}),
            Self::EmptyRevert => json!({"kind":"revert","data":""}),
        }
    }
}
/// Substituted bytes are neither executed nor read by CODECOPY on these paths.
pub fn verify_unmodified_path(e: &Execution, t: Target) -> Result<()> {
    let mut spans = vec![(t.length(false) - 53, t.length(false))];
    if t == Target::Bnbtiger {
        spans.extend([(1347, 1379), (3534, 3566)]);
    }
    for s in &e.trace {
        let end = s.pc + 1 + if (0x60..=0x7f).contains(&s.opcode) { (s.opcode - 0x5f) as usize } else { 0 };
        ensure!(spans.iter().all(|(a, b)| end <= *a || s.pc >= *b), "executed substituted opcode/immediate");
        ensure!(s.opcode != 0x39, "no CODECOPY on getter/ABI path");
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct Cell {
    pub label: String,
    pub key: U256,
    pub offset: usize,
    pub bytes: usize,
}
/// Probe each declared metadata shape at selected keys and words, including packed members, both
/// checkpoint words and hypothetical long-string data. No write admission claim.
pub fn metadata_cells(c: &Value, t: Target) -> Result<Vec<Cell>> {
    fn visit(types: &Value, kind: &str, key: U256, offset: usize, label: &str, depth: usize, out: &mut Vec<Cell>) -> Result<()> {
        ensure!(depth < 4, "bounded selected type nesting");
        let ty = &types[kind];
        match ty["encoding"].as_str().context("encoding")? {
            "mapping" => {
                let kt = ty["key"].as_str().context("key")?;
                ensure!(kt == "t_address" || kt == "t_uint32", "reviewed key width");
                let word = if kt == "t_uint32" {
                    1.into()
                } else if depth == 0 {
                    44.into()
                } else {
                    55.into()
                };
                visit(types, ty["value"].as_str().context("value")?, mapping(word, key), 0, label, depth + 1, out)?;
            }
            "bytes" => {
                ensure!(ty["label"] == "string", "only selected strings");
                out.push(Cell {
                    label: format!("{label}.head"),
                    key,
                    offset: 0,
                    bytes: 32,
                });
                out.push(Cell {
                    label: format!("{label}.hypothetical_long_data"),
                    key: vm::hash(&vm::word(key)),
                    offset: 0,
                    bytes: 32,
                });
            }
            "inplace" => {
                if let Some(members) = ty["members"].as_array() {
                    for m in members {
                        let slot = U256::from_dec_str(m["slot"].as_str().context("member slot")?)?;
                        visit(
                            types,
                            m["type"].as_str().context("member type")?,
                            key.overflowing_add(slot).0,
                            m["offset"].as_u64().context("member offset")? as usize,
                            &format!("{label}.{}", m["label"].as_str().context("label")?),
                            depth + 1,
                            out,
                        )?;
                    }
                } else {
                    let bytes = ty["numberOfBytes"].as_str().context("width")?.parse()?;
                    ensure!(bytes > 0 && bytes + offset <= 32, "selected scalar width");
                    out.push(Cell {
                        label: label.into(),
                        key,
                        offset,
                        bytes,
                    });
                }
            }
            _ => anyhow::bail!("unreviewed encoding"),
        }
        Ok(())
    }
    writers::review(c, t)?;
    let mut out = vec![];
    for f in c["storageLayout"]["storage"].as_array().context("fields")? {
        if f["label"] == "_balances" {
            continue;
        }
        visit(
            &c["storageLayout"]["types"],
            f["type"].as_str().context("type")?,
            U256::from_dec_str(f["slot"].as_str().context("slot")?)?,
            f["offset"].as_u64().context("offset")? as usize,
            f["label"].as_str().context("label")?,
            0,
            &mut out,
        )?;
    }
    Ok(out)
}
pub fn run<F: FnMut(&Value) -> Result<()>>(c: &Value, o: &Value, t: Target, save: &mut F) -> Result<Counts> {
    verify_components(c, t)?;
    check_compiled(c, &serde_json::to_vec(o)?, t)?;
    let mut p = Proof {
        t,
        codes: [
            ("compiler", bytes(&selected(o, t)["evm"]["deployedBytecode"]["object"])?),
            ("captured", runtime(c)?),
        ],
        save,
        counts: Counts::default(),
    };
    let supply: U256 = if t == Target::Bnbtiger { 25.into() } else { 3.into() };
    let base = |holder, value| {
        let mut s = State::new();
        s.insert(U256::zero(), 1.into()); // First caller is synthetic owner; second is not.
        s.insert(mapping(holder, t.root()), value);
        s.insert(supply, value);
        s
    };
    let holders = [U256::zero(), 0xdead.into(), t.account(), 44.into()];
    let values = [U256::zero(), 1.into(), 123.into(), U256::MAX];
    let callers = [1.into(), 2.into()];
    for (h, holder) in holders.iter().enumerate() {
        for (v, value) in values.iter().enumerate() {
            for (i, caller) in callers.iter().enumerate() {
                p.pair(
                    &format!("balance_h{h}_v{v}_c{i}"),
                    "balance_domain",
                    call("balanceOf(address)", &[*holder]),
                    *caller,
                    base(*holder, *value),
                    Expected::Getter(*holder, *value),
                )?;
            }
        }
    }
    for cell in metadata_cells(c, t)? {
        let mask = if cell.bytes == 32 { U256::MAX } else { (U256::one() << (cell.bytes * 8)) - 1 };
        for (v, value) in values.iter().enumerate() {
            for (i, caller) in callers.iter().enumerate() {
                let mut state = base(44.into(), 123.into());
                let existing = U256::from_big_endian(&[0xa5; 32]);
                let bitmask = mask << (cell.offset * 8);
                state.insert(cell.key, (existing & !bitmask) | ((*value & mask) << (cell.offset * 8)));
                ensure!(cell.key != mapping(44.into(), t.root()), "metadata/balance alias");
                p.pair(
                    &format!("metadata_{}_v{v}_c{i}", cell.label),
                    "metadata_independence",
                    call("balanceOf(address)", &[44.into()]),
                    *caller,
                    state,
                    Expected::Getter(44.into(), 123.into()),
                )?;
            }
        }
    }
    for caller in callers {
        let mut state = base(44.into(), 123.into());
        for h in holders {
            if h != 44.into() {
                state.insert(mapping(h, t.root()), U256::MAX);
            }
        }
        p.pair(
            "other_holders",
            "holder_independence",
            call("balanceOf(address)", &[44.into()]),
            caller,
            state,
            Expected::Getter(44.into(), 123.into()),
        )?;
    }
    let proper = call("balanceOf(address)", &[44.into()]);
    for caller in callers {
        for len in 0..proper.len() {
            p.pair(
                &format!("calldata_prefix_{len}"),
                "abi_boundary",
                proper[..len].to_vec(),
                caller,
                base(44.into(), 123.into()),
                if len == 0 { Expected::EmptyReturn } else { Expected::EmptyRevert },
            )?;
        }
        p.pair(
            "unknown_selector",
            "abi_boundary",
            vec![0xff; 36],
            caller,
            base(44.into(), 123.into()),
            Expected::EmptyRevert,
        )?;
        let mut extra = proper.clone();
        extra.extend([0xff; 33]);
        p.pair(
            "trailing_calldata",
            "abi_boundary",
            extra,
            caller,
            base(44.into(), 123.into()),
            Expected::Getter(44.into(), 123.into()),
        )?;
        let mut dirty = proper.clone();
        dirty[4] = 0x80;
        p.pair(
            "dirty_address_high_bits",
            "abi_boundary",
            dirty,
            caller,
            base(44.into(), 123.into()),
            if t == Target::Bnbtiger {
                Expected::EmptyRevert
            } else {
                Expected::Getter(44.into(), 123.into())
            },
        )?;
    }
    Ok(p.counts)
}
