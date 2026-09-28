//! Bounded host execution of the selected PToken runtime, not a general EVM.
//! No external calls, gas/refunds or fork-cost model. Unsupported execution is
//! a harness failure, never a Solidity revert. Account prestate is synthetic.
use anyhow::{bail, ensure, Context, Result};
use primitive_types::U256;
use std::collections::{BTreeMap, BTreeSet};

fn boolean(v: bool) -> U256 {
    u8::from(v).into()
}

pub type State = BTreeMap<U256, U256>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Exit {
    Return(Vec<u8>),
    Revert(Vec<u8>),
    Invalid,
    HarnessFailure(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Write {
    pub step: usize,
    pub pc: usize,
    pub key: U256,
    pub old: U256,
    pub new: U256,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read {
    pub step: usize,
    pub pc: usize,
    pub key: U256,
    pub value: U256,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Log {
    pub step: usize,
    pub pc: usize,
    pub topics: Vec<U256>,
    pub data: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keccak {
    pub step: usize,
    pub pc: usize,
    pub input: Vec<u8>,
    pub output: U256,
}
#[derive(Clone, Debug)]
pub struct Step {
    pub pc: usize,
    pub opcode: u8,
    pub stack_top: Vec<U256>,
}
#[derive(Clone, Debug)]
pub struct Execution {
    pub exit: Exit,
    pub committed: State,
    pub committed_logs: Vec<Log>,
    pub writes: Vec<Write>,
    pub reads: Vec<Read>,
    pub logs: Vec<Log>,
    pub keccaks: Vec<Keccak>,
    pub trace: Vec<Step>,
}
const MEMORY_LIMIT: usize = 1_048_576;
const STEP_LIMIT: usize = 100_000;
const WITNESS_LIMIT: usize = 8 * MEMORY_LIMIT;
pub fn word(v: U256) -> [u8; 32] {
    let mut b = [0; 32];
    v.to_big_endian(&mut b);
    b
}
pub fn hash(b: &[u8]) -> U256 {
    U256::from_big_endian(&erc20_balances::hash(b))
}
struct Machine<'a> {
    code: &'a [u8],
    data: &'a [u8],
    caller: U256,
    address: U256,
    stack: Vec<U256>,
    memory: Vec<u8>,
    state: State,
    pc: usize,
    writes: Vec<Write>,
    reads: Vec<Read>,
    logs: Vec<Log>,
    keccaks: Vec<Keccak>,
    trace: Vec<Step>,
    destinations: BTreeSet<usize>,
    witness_bytes: usize,
    witness_limit: usize,
}
impl Machine<'_> {
    fn pop(&mut self) -> Result<U256> {
        self.stack.pop().context("stack underflow")
    }
    fn push(&mut self, v: U256) -> Result<()> {
        ensure!(self.stack.len() < 1024, "stack overflow");
        self.stack.push(v);
        Ok(())
    }
    fn index(v: U256) -> Result<usize> {
        ensure!(v <= MEMORY_LIMIT.into(), "bounded index exceeded");
        Ok(v.as_usize())
    }
    fn memory(&mut self, off: usize, len: usize) -> Result<()> {
        if len == 0 {
            return Ok(());
        }
        let end = off.checked_add(len).context("memory arithmetic overflow")?;
        ensure!(end <= MEMORY_LIMIT, "memory bound exceeded");
        if end > self.memory.len() {
            self.memory.resize(end.div_ceil(32) * 32, 0)
        }
        Ok(())
    }
    fn slice(&mut self, off: U256, len: usize) -> Result<Vec<u8>> {
        // Zero-sized memory operations do not inspect or expand their offset.
        if len == 0 {
            return Ok(vec![]);
        }
        let off = Self::index(off)?;
        self.memory(off, len)?;
        self.witness_bytes = self.witness_bytes.checked_add(len).context("witness size overflow")?;
        ensure!(self.witness_bytes <= self.witness_limit, "witness byte bound exceeded");
        Ok(self.memory[off..off + len].to_vec())
    }
    fn jump(&mut self, target: U256) -> Result<()> {
        let target = Self::index(target)?;
        ensure!(self.destinations.contains(&target), "invalid jump destination {target}");
        self.pc = target;
        Ok(())
    }
    fn run(&mut self) -> Result<Exit> {
        for step in 1..=STEP_LIMIT {
            // EVM falling off code is STOP; truncated PUSH data is zero-padded.
            if self.pc >= self.code.len() {
                return Ok(Exit::Return(vec![]));
            }
            let pc = self.pc;
            let op = self.code[pc];
            self.pc += 1;
            self.trace.push(Step {
                pc,
                opcode: op,
                stack_top: self.stack.iter().rev().take(8).copied().collect(),
            });
            match op {
                0x00 => return Ok(Exit::Return(vec![])),
                0x01 | 0x02 | 0x03 | 0x04 | 0x06 | 0x0a | 0x0b | 0x10..=0x14 | 0x16..=0x18 => {
                    let a = self.pop()?;
                    let b = self.pop()?;
                    let value = match op {
                        0x01 => a.overflowing_add(b).0,
                        0x02 => a.overflowing_mul(b).0,
                        0x03 => a.overflowing_sub(b).0,
                        0x04 => {
                            if b.is_zero() {
                                U256::zero()
                            } else {
                                a / b
                            }
                        }
                        0x06 => {
                            if b.is_zero() {
                                U256::zero()
                            } else {
                                a % b
                            }
                        }
                        0x0a => {
                            let (mut base, mut exponent, mut value) = (a, b, U256::one());
                            while !exponent.is_zero() {
                                if exponent.bit(0) {
                                    value = value.overflowing_mul(base).0;
                                }
                                base = base.overflowing_mul(base).0;
                                exponent >>= 1;
                            }
                            value
                        }
                        0x0b => {
                            if a >= 31.into() {
                                b
                            } else {
                                let bit = a.as_usize() * 8 + 7;
                                let mask = (U256::one() << (bit + 1)) - U256::one();
                                if b.bit(bit) {
                                    b | !mask
                                } else {
                                    b & mask
                                }
                            }
                        }
                        0x10 => boolean(a < b),
                        0x11 => boolean(a > b),
                        0x12 | 0x13 => {
                            let (an, bn) = (a.bit(255), b.bit(255));
                            boolean(if an == bn {
                                if op == 0x12 {
                                    a < b
                                } else {
                                    a > b
                                }
                            } else if op == 0x12 {
                                an
                            } else {
                                bn
                            })
                        }
                        0x14 => boolean(a == b),
                        0x16 => a & b,
                        0x17 => a | b,
                        0x18 => a ^ b,
                        _ => unreachable!(),
                    };
                    self.push(value)?;
                }
                0x15 => {
                    let v = self.pop()?;
                    self.push(boolean(v.is_zero()))?;
                }
                0x19 => {
                    let v = self.pop()?;
                    self.push(!v)?;
                }
                0x1a => {
                    let i = self.pop()?;
                    let v = self.pop()?;
                    self.push(if i >= 32.into() {
                        U256::zero()
                    } else {
                        (v >> (8 * (31 - i.as_usize()))) & 255.into()
                    })?;
                }
                0x1b..=0x1d => {
                    let n = self.pop()?;
                    let v = self.pop()?;
                    let negative = op == 0x1d && v.bit(255);
                    let value = if n >= 256.into() {
                        if negative {
                            U256::max_value()
                        } else {
                            U256::zero()
                        }
                    } else {
                        let n = n.as_usize();
                        if op == 0x1b {
                            v << n
                        } else if negative && n > 0 {
                            (v >> n) | (U256::max_value() << (256 - n))
                        } else {
                            v >> n
                        }
                    };
                    self.push(value)?;
                }
                0x20 => {
                    let off = self.pop()?;
                    let len = Self::index(self.pop()?)?;
                    let input = self.slice(off, len)?;
                    let output = hash(&input);
                    self.keccaks.push(Keccak { step, pc, input, output });
                    self.push(output)?;
                }
                0x30 => self.push(self.address)?,
                0x33 => self.push(self.caller)?,
                0x34 | 0x3d => self.push(U256::zero())?,
                0x35 => {
                    let off = self.pop()?;
                    let mut data = [0; 32];
                    if off <= usize::MAX.into() {
                        let off = off.as_usize();
                        for (i, b) in data.iter_mut().enumerate() {
                            *b = off.checked_add(i).and_then(|n| self.data.get(n)).copied().unwrap_or(0);
                        }
                    }
                    self.push(U256::from_big_endian(&data))?;
                }
                0x36 => self.push(self.data.len().into())?,
                0x38 => self.push(self.code.len().into())?,
                0x37 | 0x39 => {
                    let dst = self.pop()?;
                    let src = self.pop()?;
                    let len = Self::index(self.pop()?)?;
                    if len > 0 {
                        let dst = Self::index(dst)?;
                        self.memory(dst, len)?;
                        let input = if op == 0x37 { self.data } else { self.code };
                        for i in 0..len {
                            self.memory[dst + i] = if src <= usize::MAX.into() {
                                src.as_usize().checked_add(i).and_then(|n| input.get(n)).copied().unwrap_or(0)
                            } else {
                                0
                            };
                        }
                    }
                }
                0x50 => {
                    self.pop()?;
                }
                0x51 => {
                    let off = Self::index(self.pop()?)?;
                    self.memory(off, 32)?;
                    self.push(U256::from_big_endian(&self.memory[off..off + 32]))?;
                }
                0x52 | 0x53 => {
                    let off = Self::index(self.pop()?)?;
                    let v = self.pop()?;
                    let len = if op == 0x52 { 32 } else { 1 };
                    self.memory(off, len)?;
                    if len == 32 {
                        self.memory[off..off + 32].copy_from_slice(&word(v))
                    } else {
                        self.memory[off] = v.low_u32() as u8
                    }
                }
                0x54 => {
                    let key = self.pop()?;
                    let value = self.state.get(&key).copied().unwrap_or_default();
                    self.reads.push(Read { step, pc, key, value });
                    self.push(value)?;
                }
                0x55 => {
                    let key = self.pop()?;
                    let new = self.pop()?;
                    let old = self.state.insert(key, new).unwrap_or_default();
                    self.writes.push(Write { step, pc, key, old, new });
                }
                0x56 => {
                    let dest = self.pop()?;
                    self.jump(dest)?;
                }
                0x57 => {
                    let dest = self.pop()?;
                    let condition = self.pop()?;
                    if !condition.is_zero() {
                        self.jump(dest)?;
                    }
                }
                0x58 => self.push(pc.into())?,
                0x59 => self.push(self.memory.len().into())?,
                0x5b => {}
                0x5e => {
                    let dst = self.pop()?;
                    let src = self.pop()?;
                    let len = Self::index(self.pop()?)?;
                    if len > 0 {
                        let (dst, src) = (Self::index(dst)?, Self::index(src)?);
                        self.memory(dst.max(src), len)?;
                        self.memory.copy_within(src..src + len, dst);
                    }
                }
                0x5f => self.push(U256::zero())?,
                0x60..=0x7f => {
                    let n = usize::from(op - 0x5f);
                    let mut data = [0; 32];
                    for i in 0..n {
                        data[32 - n + i] = self.code.get(self.pc + i).copied().unwrap_or(0)
                    }
                    self.pc += n;
                    self.push(U256::from_big_endian(&data))?;
                }
                0x80..=0x8f => {
                    let n = usize::from(op - 0x7f);
                    ensure!(self.stack.len() >= n, "DUP underflow");
                    self.push(self.stack[self.stack.len() - n])?;
                }
                0x90..=0x9f => {
                    let n = usize::from(op - 0x8f);
                    ensure!(self.stack.len() > n, "SWAP underflow");
                    let last = self.stack.len() - 1;
                    self.stack.swap(last, last - n);
                }
                0xa0..=0xa4 => {
                    let off = self.pop()?;
                    let len = Self::index(self.pop()?)?;
                    let mut topics = vec![];
                    for _ in 0..op - 0xa0 {
                        topics.push(self.pop()?)
                    }
                    let data = self.slice(off, len)?;
                    self.logs.push(Log { step, pc, topics, data });
                }
                0xf3 | 0xfd => {
                    let off = self.pop()?;
                    let len = Self::index(self.pop()?)?;
                    let data = self.slice(off, len)?;
                    return Ok(if op == 0xf3 { Exit::Return(data) } else { Exit::Revert(data) });
                }
                0xfe => return Ok(Exit::Invalid),
                _ => bail!("unsupported opcode 0x{op:02x} at pc {pc}"),
            }
        }
        bail!("step bound exceeded")
    }
}
pub fn execute(code: &[u8], data: &[u8], caller: U256, address: U256, prestate: &State) -> Execution {
    execute_with_limit(code, data, caller, address, prestate, WITNESS_LIMIT)
}
fn execute_with_limit(code: &[u8], data: &[u8], caller: U256, address: U256, prestate: &State, witness_limit: usize) -> Execution {
    let mut destinations = BTreeSet::new();
    let mut pc = 0;
    while pc < code.len() {
        let op = code[pc];
        if op == 0x5b {
            destinations.insert(pc);
        }
        pc += 1 + if (0x60..=0x7f).contains(&op) { usize::from(op - 0x5f) } else { 0 };
    }
    let mut vm = Machine {
        code,
        data,
        caller,
        address,
        stack: vec![],
        memory: vec![],
        state: prestate.clone(),
        pc: 0,
        writes: vec![],
        reads: vec![],
        logs: vec![],
        keccaks: vec![],
        trace: vec![],
        destinations,
        witness_bytes: 0,
        witness_limit,
    };
    let exit = vm.run().unwrap_or_else(|e| Exit::HarnessFailure(format!("{e:#}")));
    let success = matches!(exit, Exit::Return(_));
    Execution {
        exit,
        committed: if success { vm.state } else { prestate.clone() },
        committed_logs: if success { vm.logs.clone() } else { vec![] },
        writes: vm.writes,
        reads: vm.reads,
        logs: vm.logs,
        keccaks: vm.keccaks,
        trace: vm.trace,
    }
}

#[cfg(test)]
pub(super) fn small_witness_execution(code: &[u8], prestate: &State) -> Execution {
    execute_with_limit(code, &[], 0.into(), 0.into(), prestate, 64)
}
