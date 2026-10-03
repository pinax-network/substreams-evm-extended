//! Runs a packaged map module the way the Substreams runtime calls one: each
//! input is copied into guest memory through the exported `alloc`, the
//! entrypoint receives `(ptr, len)` pairs and the result arrives through
//! `env.output`. Each call gets a fresh instance, as each block does.

use std::{collections::BTreeSet, path::Path};

use anyhow::{anyhow, Context, Result};
use wasmi::{Caller, Engine, Extern, Linker, Module, Store, Val};

/// Everything one invocation produced. Two packages agree on an input when
/// their outcomes are equal (see [`Outcome::same_as`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    pub output: Option<Vec<u8>>,
    pub skipped_empty_output: bool,
    pub logs: Vec<String>,
    pub panic: Option<Panic>,
    pub trapped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Panic {
    pub message: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
}

impl Outcome {
    /// Equality of output, logs and panic message. Panic locations are not
    /// compared: the native `db_out` drops the ERC-20 lines of the upstream
    /// `lib.rs`, which moves the same `expect` to another line.
    pub fn same_as(&self, other: &Outcome) -> bool {
        let message = |p: &Option<Panic>| p.as_ref().map(|p| (p.message.clone(), file_name(&p.file)));
        self.output == other.output
            && self.skipped_empty_output == other.skipped_empty_output
            && self.logs == other.logs
            && self.trapped == other.trapped
            && message(&self.panic) == message(&other.panic)
    }
}

fn file_name(path: &str) -> String {
    Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

#[derive(Default)]
struct Host {
    outcome: Outcome,
}

pub struct MapModule {
    engine: Engine,
    module: Module,
    linker: Linker<Host>,
    entrypoint: String,
}

impl MapModule {
    pub fn new(wasm: &[u8], entrypoint: &str) -> Result<Self> {
        let engine = Engine::default();
        let module = Module::new(&engine, wasm).map_err(|e| anyhow!("compiling {entrypoint}: {e}"))?;
        let mut linker = Linker::new(&engine);
        linker.func_wrap(
            "env",
            "output",
            |mut caller: Caller<'_, Host>, ptr: i32, len: i32| -> Result<(), wasmi::Error> {
                let bytes = read(&caller, ptr, len)?;
                caller.data_mut().outcome.output = Some(bytes);
                Ok(())
            },
        )?;
        linker.func_wrap("env", "skip_empty_output", |mut caller: Caller<'_, Host>| {
            caller.data_mut().outcome.skipped_empty_output = true;
        })?;
        linker.func_wrap(
            "logger",
            "println",
            |mut caller: Caller<'_, Host>, ptr: i32, len: i32| -> Result<(), wasmi::Error> {
                let line = String::from_utf8_lossy(&read(&caller, ptr, len)?).into_owned();
                caller.data_mut().outcome.logs.push(line);
                Ok(())
            },
        )?;
        linker.func_wrap(
            "env",
            "register_panic",
            |mut caller: Caller<'_, Host>, msg_ptr: i32, msg_len: i32, file_ptr: i32, file_len: i32, line: i32, column: i32| -> Result<(), wasmi::Error> {
                let message = String::from_utf8_lossy(&read(&caller, msg_ptr, msg_len)?).into_owned();
                let file = String::from_utf8_lossy(&read(&caller, file_ptr, file_len)?).into_owned();
                caller.data_mut().outcome.panic = Some(Panic {
                    message,
                    file,
                    line: line as u32,
                    column: column as u32,
                });
                Ok(())
            },
        )?;
        Ok(MapModule {
            engine,
            module,
            linker,
            entrypoint: entrypoint.to_string(),
        })
    }

    /// Calls the entrypoint with `inputs` in manifest order.
    pub fn call(&self, inputs: &[&[u8]]) -> Result<Outcome> {
        let mut store = Store::new(&self.engine, Host::default());
        let instance = self
            .linker
            .instantiate_and_start(&mut store, &self.module)
            .map_err(|e| anyhow!("instantiating: {e}"))?;
        let memory = instance.get_memory(&store, "memory").context("guest exports no memory")?;
        let alloc = instance.get_typed_func::<i32, i32>(&store, "alloc").map_err(|e| anyhow!("alloc: {e}"))?;
        let mut args = Vec::with_capacity(inputs.len() * 2);
        for input in inputs {
            let len = i32::try_from(input.len()).context("input exceeds 32-bit guest memory")?;
            let ptr = alloc.call(&mut store, len).map_err(|e| anyhow!("alloc({len}): {e}"))?;
            memory
                .write(&mut store, ptr as u32 as usize, input)
                .map_err(|e| anyhow!("writing input: {e}"))?;
            args.extend([Val::I32(ptr), Val::I32(len)]);
        }
        let entrypoint = instance
            .get_export(&store, &self.entrypoint)
            .and_then(Extern::into_func)
            .with_context(|| format!("guest exports no {:?}", self.entrypoint))?;
        let trapped = entrypoint.call(&mut store, &args, &mut []).is_err();
        let mut outcome = std::mem::take(&mut store.data_mut().outcome);
        outcome.trapped = trapped;
        Ok(outcome)
    }
}

/// Host modules a binary imports from, e.g. `env`, `logger` or `rpc`.
pub fn import_modules(wasm: &[u8]) -> Result<BTreeSet<String>> {
    let module = Module::new(&Engine::default(), wasm).map_err(|e| anyhow!("parsing module: {e}"))?;
    Ok(module.imports().map(|import| import.module().to_string()).collect())
}

fn read(caller: &Caller<'_, Host>, ptr: i32, len: i32) -> Result<Vec<u8>, wasmi::Error> {
    let memory = caller
        .get_export("memory")
        .and_then(Extern::into_memory)
        .ok_or_else(|| wasmi::Error::new("guest exports no memory"))?;
    let start = ptr as u32 as usize;
    let end = start
        .checked_add(len as u32 as usize)
        .ok_or_else(|| wasmi::Error::new("guest range overflows"))?;
    memory
        .data(caller)
        .get(start..end)
        .map(<[u8]>::to_vec)
        .ok_or_else(|| wasmi::Error::new("guest range outside memory"))
}
