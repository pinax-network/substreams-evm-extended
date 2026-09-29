//! Opt-in, bounded synthetic external reads. No callee execution or gas model.
//! Existing VM entrypoints never install this context or change their JSON.
use super::*;

const MAX_CALLS: usize = 2;
const MAX_BYTES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScriptedResult {
    Return(Vec<u8>),
    Revert(Vec<u8>),
}
impl ScriptedResult {
    pub fn data(&self) -> &[u8] {
        match self {
            Self::Return(v) | Self::Revert(v) => v,
        }
    }
}
#[derive(Clone, Debug)]
pub struct GasExpectation {
    pub pc: usize,
    pub value: U256,
}
#[derive(Clone, Debug)]
pub struct StaticCallExpectation {
    pub pc: usize,
    pub address: U256,
    pub gas: U256,
    pub input: Vec<u8>,
    pub output_size: usize,
    pub result: ScriptedResult,
}
#[derive(Clone, Debug)]
pub struct ReadOnlyContext {
    pub origin: U256,
    pub timestamp: Option<U256>,
    pub gas_reads: Vec<GasExpectation>,
    pub calls: Vec<StaticCallExpectation>,
}
impl ReadOnlyContext {
    fn validate(&self) -> Result<()> {
        ensure!(self.origin < (U256::one() << 160), "canonical explicit ORIGIN");
        ensure!(
            self.calls.len() <= MAX_CALLS && self.gas_reads.len() <= MAX_CALLS,
            "bounded external script count"
        );
        for call in &self.calls {
            ensure!(call.address < (U256::one() << 160), "canonical expected external address");
            ensure!(
                call.input.len() <= MAX_BYTES && call.output_size <= MAX_BYTES && call.result.data().len() <= MAX_BYTES,
                "bounded external script bytes"
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextRead {
    pub step: usize,
    pub pc: usize,
    pub value: U256,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticCallWitness {
    pub step: usize,
    pub pc: usize,
    pub raw_address: U256,
    pub address: U256,
    pub gas: U256,
    pub input_offset: U256,
    pub input_size: U256,
    pub output_offset: U256,
    pub output_size: U256,
    pub input: Vec<u8>,
    /// None records an attempted interaction refused by the script boundary.
    pub result: Option<ScriptedResult>,
}
#[derive(Clone, Debug)]
pub struct ContextExecution {
    pub execution: Execution,
    pub calls: Vec<StaticCallWitness>,
    pub origin_reads: Vec<ContextRead>,
    pub gas_reads: Vec<ContextRead>,
    pub consumed_calls: usize,
    pub consumed_gas: usize,
}
impl ContextExecution {
    pub(super) fn new(execution: Execution, context: Option<ReadMachine>) -> Self {
        let mut out = Self {
            execution,
            calls: vec![],
            origin_reads: vec![],
            gas_reads: vec![],
            consumed_calls: 0,
            consumed_gas: 0,
        };
        if let Some(c) = context {
            out.calls = c.calls;
            out.origin_reads = c.origin_reads;
            out.gas_reads = c.gas_reads;
            out.consumed_calls = c.next_call;
            out.consumed_gas = c.next_gas;
        }
        out
    }
}
pub(super) struct ReadMachine {
    script: ReadOnlyContext,
    pub(super) return_data: Vec<u8>,
    next_call: usize,
    next_gas: usize,
    calls: Vec<StaticCallWitness>,
    origin_reads: Vec<ContextRead>,
    gas_reads: Vec<ContextRead>,
}
impl ReadMachine {
    pub(super) fn new(script: ReadOnlyContext) -> Self {
        Self {
            script,
            return_data: vec![],
            next_call: 0,
            next_gas: 0,
            calls: vec![],
            origin_reads: vec![],
            gas_reads: vec![],
        }
    }
    pub(super) fn complete(&self) -> Result<()> {
        ensure!(
            self.next_call == self.script.calls.len() && self.next_gas == self.script.gas_reads.len(),
            "unconsumed external/GAS script"
        );
        Ok(())
    }
}
/// No external code, gas sufficiency, fork costs or nonzero call value is modeled.
/// Unknown interactions and incomplete scripts are harness failures, never
/// synthetic source reverts that Solidity try/catch could silently absorb.
pub fn execute_with_read_only_context(code: &[u8], data: &[u8], caller: U256, address: U256, prestate: &State, context: &ReadOnlyContext) -> ContextExecution {
    if let Err(e) = context.validate() {
        return ContextExecution::new(
            Execution {
                exit: Exit::HarnessFailure(format!("{e:#}")),
                committed: prestate.clone(),
                committed_logs: vec![],
                writes: vec![],
                reads: vec![],
                logs: vec![],
                keccaks: vec![],
                trace: vec![],
            },
            None,
        );
    }
    execute_contextual(
        code,
        data,
        caller,
        address,
        prestate,
        WITNESS_LIMIT,
        Environment {
            self_code_size: None,
            timestamp: context.timestamp,
            read_only: Some(context.clone()),
        },
    )
}
impl Machine<'_> {
    pub(super) fn read_origin(&mut self, step: usize, pc: usize) -> Result<()> {
        let c = self.external.as_mut().context("explicit external context")?;
        let value = c.script.origin;
        c.origin_reads.push(ContextRead { step, pc, value });
        self.push(value)
    }
    pub(super) fn read_gas(&mut self, step: usize, pc: usize) -> Result<()> {
        let c = self.external.as_mut().context("explicit external context")?;
        let expected = c.script.gas_reads.get(c.next_gas).context("unexpected GAS read")?;
        ensure!(pc == expected.pc, "unexpected GAS PC");
        let value = expected.value;
        c.next_gas += 1;
        c.gas_reads.push(ContextRead { step, pc, value });
        self.push(value)
    }
    pub(super) fn scripted_staticcall(&mut self, step: usize, pc: usize) -> Result<()> {
        let gas = self.pop()?;
        let raw_address = self.pop()?;
        let address = raw_address & ((U256::one() << 160) - U256::one());
        let input_offset = self.pop()?;
        let input_size = self.pop()?;
        let output_offset = self.pop()?;
        let output_size = self.pop()?;
        ensure!(input_size <= MAX_BYTES.into() && output_size <= MAX_BYTES.into(), "bounded STATICCALL ranges");
        let input = self.slice(input_offset, input_size.as_usize())?;
        let c = self.external.as_mut().context("explicit external context")?;
        c.calls.push(StaticCallWitness {
            step,
            pc,
            raw_address,
            address,
            gas,
            input_offset,
            input_size,
            output_offset,
            output_size,
            input: input.clone(),
            result: None,
        });
        let expected = c.script.calls.get(c.next_call).context("unexpected STATICCALL")?;
        ensure!(
            expected.pc == pc
                && expected.address == address
                && expected.gas == gas
                && expected.input == input
                && U256::from(expected.output_size) == output_size,
            "STATICCALL script mismatch"
        );
        let result = expected.result.clone();
        self.witness_bytes = self.witness_bytes.checked_add(result.data().len()).context("witness size overflow")?;
        ensure!(self.witness_bytes <= self.witness_limit, "witness byte bound exceeded");
        if !output_size.is_zero() {
            let dst = Self::index(output_offset)?;
            self.memory(dst, output_size.as_usize())?;
            let n = output_size.as_usize().min(result.data().len());
            self.memory[dst..dst + n].copy_from_slice(&result.data()[..n]);
        }
        let success = matches!(result, ScriptedResult::Return(_));
        let c = self.external.as_mut().context("explicit external context")?;
        c.return_data = result.data().to_vec();
        c.calls.last_mut().context("call witness")?.result = Some(result);
        c.next_call += 1;
        self.push(if success { U256::one() } else { U256::zero() })
    }
    pub(super) fn copy_return_data(&mut self) -> Result<()> {
        let dst = self.pop()?;
        let src = self.pop()?;
        let len = self.pop()?;
        let data = &self.external.as_ref().context("explicit external context")?.return_data;
        let (end, overflow) = src.overflowing_add(len);
        ensure!(!overflow && end <= data.len().into(), "bounded VM refuses RETURNDATACOPY out of bounds");
        if len.is_zero() {
            return Ok(());
        }
        // The source bound above limits both integer conversions to MAX_BYTES.
        let data = data[src.as_usize()..end.as_usize()].to_vec();
        self.witness_bytes = self.witness_bytes.checked_add(data.len()).context("witness size overflow")?;
        ensure!(self.witness_bytes <= self.witness_limit, "witness byte bound exceeded");
        let dst = Self::index(dst)?;
        self.memory(dst, data.len())?;
        self.memory[dst..dst + data.len()].copy_from_slice(&data);
        Ok(())
    }
}
