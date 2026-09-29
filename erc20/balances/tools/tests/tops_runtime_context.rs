#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::ptoken_proof::vm::*;
use primitive_types::U256;

fn push(c: &mut Vec<u8>, v: U256) {
    c.push(0x7f);
    c.extend(word(v));
}
fn store(c: &mut Vec<u8>, at: usize, v: U256) {
    push(c, v);
    push(c, at.into());
    c.push(0x52);
}
fn ret(c: &mut Vec<u8>, at: usize, len: usize) {
    push(c, len.into());
    push(c, at.into());
    c.push(0xf3);
}
fn context() -> ReadOnlyContext {
    ReadOnlyContext {
        origin: 17.into(),
        timestamp: Some(U256::MAX),
        gas_reads: vec![],
        calls: vec![],
    }
}
fn run(code: &[u8], c: &ReadOnlyContext) -> ContextExecution {
    execute_with_read_only_context(code, &[], 1.into(), 2.into(), &State::new(), c)
}
fn returned(e: &ContextExecution) -> Vec<u8> {
    match &e.execution.exit {
        Exit::Return(v) => v.clone(),
        other => panic!("{other:?}"),
    }
}
fn refused(e: &ContextExecution) {
    assert!(matches!(e.execution.exit, Exit::HarnessFailure(_)), "{:?}", e.execution.exit);
}
#[allow(clippy::too_many_arguments)]
fn call(
    code: &mut Vec<u8>,
    c: &mut ReadOnlyContext,
    raw_address: U256,
    input: &[u8],
    input_at: U256,
    output_at: U256,
    output_size: usize,
    result: ScriptedResult,
) {
    if !input.is_empty() {
        for (i, part) in input.chunks(32).enumerate() {
            let mut b = [0; 32];
            b[..part.len()].copy_from_slice(part);
            store(code, input_at.as_usize() + 32 * i, U256::from_big_endian(&b));
        }
    }
    for v in [
        U256::from(output_size),
        output_at,
        U256::from(input.len()),
        input_at,
        raw_address,
        U256::from(99),
    ] {
        push(code, v);
    }
    let pc = code.len();
    code.push(0xfa);
    c.calls.push(StaticCallExpectation {
        pc,
        address: raw_address & ((U256::one() << 160) - U256::one()),
        gas: 99.into(),
        input: input.to_vec(),
        output_size,
        result,
    });
}
#[test]
fn origin_timestamp_and_return_data_start_are_explicit() {
    let mut code = vec![0x32];
    push(&mut code, 0.into());
    code.push(0x52);
    code.push(0x32);
    push(&mut code, 32.into());
    code.push(0x52);
    code.push(0x42);
    push(&mut code, 64.into());
    code.push(0x52);
    code.push(0x3d);
    push(&mut code, 96.into());
    code.push(0x52);
    ret(&mut code, 0, 128);
    let c = context();
    let e = run(&code, &c);
    assert_eq!(returned(&e), [word(c.origin), word(c.origin), word(U256::MAX), word(0.into())].concat());
    assert_eq!(e.origin_reads.len(), 2);
    let mut c = c;
    c.origin = U256::one() << 160;
    refused(&run(&code, &c));
}
#[test]
fn gas_words_are_full_width_ordered_and_not_a_gas_model() {
    let mut code = vec![];
    let mut c = context();
    for (i, v) in [U256::zero(), U256::MAX].into_iter().enumerate() {
        c.gas_reads.push(GasExpectation { pc: code.len(), value: v });
        code.push(0x5a);
        push(&mut code, (i * 32).into());
        code.push(0x52);
    }
    ret(&mut code, 0, 64);
    let e = run(&code, &c);
    assert_eq!(returned(&e), [word(0.into()), word(U256::MAX)].concat());
    assert_eq!(e.consumed_gas, 2);
    let mut wrong = c.clone();
    wrong.gas_reads[0].pc += 1;
    refused(&run(&code, &wrong));
    wrong = c.clone();
    wrong.gas_reads.pop();
    refused(&run(&code, &wrong));
}
#[test]
fn short_static_return_preserves_output_suffix_and_masks_address() {
    let mut code = vec![];
    store(&mut code, 64, U256::from_big_endian(&[0xa5; 32]));
    let mut c = context();
    call(
        &mut code,
        &mut c,
        (U256::one() << 255) + U256::from(9),
        &[1, 2, 3, 4],
        0.into(),
        64.into(),
        32,
        ScriptedResult::Return(vec![7, 8, 9]),
    );
    code.push(0x50);
    ret(&mut code, 64, 32);
    let e = run(&code, &c);
    let mut expected = vec![0xa5; 32];
    expected[..3].copy_from_slice(&[7, 8, 9]);
    assert_eq!(returned(&e), expected);
    assert_eq!(e.calls[0].address, U256::from(9));
    assert_ne!(e.calls[0].raw_address, e.calls[0].address);
    assert_eq!(e.consumed_calls, 1);
}
#[test]
fn full_return_buffer_is_distinct_from_truncated_output_memory() {
    let mut code = vec![];
    let mut c = context();
    let data = (0..64).collect::<Vec<_>>();
    call(&mut code, &mut c, 9.into(), &[], U256::MAX, 64.into(), 3, ScriptedResult::Return(data.clone()));
    code.push(0x50);
    for v in [64.into(), 0.into(), 128.into()] {
        push(&mut code, v);
    }
    code.push(0x3e);
    ret(&mut code, 128, 64);
    let e = run(&code, &c);
    assert_eq!(returned(&e), data);
    assert!(e.calls[0].input.is_empty());
}
#[test]
fn two_calls_replace_last_return_data_including_empty_return() {
    let mut code = vec![];
    let mut c = context();
    call(&mut code, &mut c, 9.into(), &[], U256::MAX, U256::MAX, 0, ScriptedResult::Return(vec![9; 64]));
    code.push(0x50);
    call(&mut code, &mut c, 10.into(), &[], U256::MAX, U256::MAX, 0, ScriptedResult::Return(vec![]));
    code.push(0x50);
    code.push(0x3d);
    push(&mut code, 0.into());
    code.push(0x52);
    ret(&mut code, 0, 32);
    let e = run(&code, &c);
    assert_eq!(returned(&e), word(0.into()));
    assert_eq!(e.consumed_calls, 2);
}
#[test]
fn explicit_revert_payload_bubbles_through_original_return_data_opcodes() {
    let mut code = vec![];
    let mut c = context();
    let payload = vec![0xde, 0xad, 0xbe, 0xef];
    call(
        &mut code,
        &mut c,
        9.into(),
        &[],
        U256::MAX,
        64.into(),
        32,
        ScriptedResult::Revert(payload.clone()),
    );
    code.push(0x50);
    code.extend([0x3d, 0x80, 0x5f, 0x5f, 0x3e, 0x5f, 0xfd]);
    let e = run(&code, &c);
    assert_eq!(e.execution.exit, Exit::Revert(payload));
    assert_eq!(e.consumed_calls, 1);
    assert!(e.execution.committed_logs.is_empty());
}
#[test]
fn return_data_zero_length_still_checks_source_offset() {
    for (src, valid) in [(0, true), (1, false)] {
        let mut code = vec![];
        for v in [U256::zero(), src.into(), U256::MAX] {
            push(&mut code, v);
        }
        code.push(0x3e);
        code.push(0x00);
        let e = run(&code, &context());
        if valid {
            assert!(returned(&e).is_empty());
        } else {
            refused(&e);
        }
    }
    let mut code = vec![];
    for v in [U256::one(), U256::MAX, U256::zero()] {
        push(&mut code, v);
    }
    code.push(0x3e);
    refused(&run(&code, &context()));
}
#[test]
fn overlapping_call_input_and_output_preserves_captured_input() {
    let mut code = vec![];
    let mut c = context();
    call(
        &mut code,
        &mut c,
        9.into(),
        &[1; 36],
        0.into(),
        0.into(),
        32,
        ScriptedResult::Return(vec![2; 32]),
    );
    code.push(0x50);
    ret(&mut code, 0, 36);
    let e = run(&code, &c);
    assert_eq!(e.calls[0].input, vec![1; 36]);
    assert_eq!(returned(&e), [vec![2; 32], vec![1; 4]].concat());
}
#[test]
fn call_script_rejects_each_identity_mismatch_and_unlisted_call() {
    let mut code = vec![];
    let mut good = context();
    call(
        &mut code,
        &mut good,
        9.into(),
        &[1, 2, 3, 4],
        0.into(),
        64.into(),
        32,
        ScriptedResult::Return(vec![0; 32]),
    );
    code.push(0x00);
    assert_eq!(run(&code, &good).consumed_calls, 1);
    for n in 0..6 {
        let mut bad = good.clone();
        match n {
            0 => bad.calls[0].pc += 1,
            1 => bad.calls[0].address = 10.into(),
            2 => bad.calls[0].gas = 100.into(),
            3 => bad.calls[0].input[0] ^= 1,
            4 => bad.calls[0].output_size = 31,
            _ => bad.calls.clear(),
        };
        let e = run(&code, &bad);
        refused(&e);
        assert_eq!(e.calls.len(), 1);
        assert_eq!(e.calls[0].result, None);
    }
}
#[test]
fn missing_script_consumption_is_failure_even_after_source_revert() {
    let expected = StaticCallExpectation {
        pc: 0,
        address: 9.into(),
        gas: 99.into(),
        input: vec![],
        output_size: 0,
        result: ScriptedResult::Return(vec![]),
    };
    for code in [vec![0], vec![0x5f, 0x5f, 0xfd]] {
        let mut c = context();
        c.calls.push(expected.clone());
        refused(&run(&code, &c));
        c.calls.clear();
        c.gas_reads.push(GasExpectation { pc: 0, value: 1.into() });
        refused(&run(&code, &c));
    }
}
#[test]
fn failed_call_and_unsupported_stateful_paths_rollback_local_effects() {
    let pre = State::from([(7.into(), 9.into())]);
    for op in [0xf1, 0xf2, 0xf4, 0xf0, 0xf5, 0xff, 0x3b] {
        let mut code = hex::decode("60016007555f5fa0").unwrap();
        code.push(op);
        let e = execute_with_read_only_context(&code, &[], 1.into(), 2.into(), &pre, &context());
        refused(&e);
        assert_eq!(e.execution.writes.len(), 1);
        assert_eq!(e.execution.logs.len(), 1);
        assert_eq!(e.execution.committed, pre);
        assert!(e.execution.committed_logs.is_empty());
    }
}
#[test]
fn bounded_scripts_and_copy_witnesses_fail_closed() {
    let mut c = context();
    let item = StaticCallExpectation {
        pc: 0,
        address: 9.into(),
        gas: 0.into(),
        input: vec![],
        output_size: 0,
        result: ScriptedResult::Return(vec![]),
    };
    c.calls = vec![item.clone(); 3];
    refused(&run(&[0], &c));
    c.calls = vec![item];
    c.calls[0].result = ScriptedResult::Return(vec![1; 4097]);
    refused(&run(&[0], &c));
    let mut c = context();
    let mut code = vec![];
    call(&mut code, &mut c, 9.into(), &[], U256::MAX, U256::MAX, 0, ScriptedResult::Return(vec![1; 4096]));
    code.push(0x50);
    let loop_pc = code.len();
    code.push(0x5b);
    for v in [4096.into(), 0.into(), 0.into()] {
        push(&mut code, v);
    }
    code.push(0x3e);
    push(&mut code, loop_pc.into());
    code.push(0x56);
    let e = run(&code, &c);
    assert!(matches!(e.execution.exit,Exit::HarnessFailure(ref s) if s.contains("witness byte bound exceeded")));
    assert_eq!(e.consumed_calls, 1);
}
#[test]
fn legacy_entrypoints_and_default_return_size_are_unchanged() {
    for (op, text) in [(0x32, "0x32"), (0x5a, "0x5a"), (0xfa, "0xfa"), (0x3e, "0x3e")] {
        for e in [
            execute(&[op], &[], 1.into(), 2.into(), &State::new()),
            execute_with_timestamp(&[op], &[], 1.into(), 2.into(), &State::new(), Some(1.into())),
            execute_with_self_code_size(&[op], &[], 1.into(), 2.into(), &State::new(), Some(9)),
        ] {
            assert!(matches!(e.exit,Exit::HarnessFailure(ref s) if s==&format!("unsupported opcode {text} at pc 0")));
        }
    }
    let code = hex::decode("3d5f5260205ff3").unwrap();
    assert_eq!(
        execute(&code, &[], 1.into(), 2.into(), &State::new()).exit,
        Exit::Return(word(0.into()).to_vec())
    );
}
#[test]
fn stack_status_and_nonempty_return_data_edges_preserve_rollback() {
    for opcode in [0xfa, 0x3e] {
        let mut code = vec![];
        push(&mut code, 9.into());
        push(&mut code, 7.into());
        code.push(0x55);
        push(&mut code, 0.into());
        push(&mut code, 0.into());
        code.push(0xa0);
        code.push(opcode);
        let pre = State::from([(7.into(), 3.into())]);
        let e = execute_with_read_only_context(&code, &[], 1.into(), 2.into(), &pre, &context());
        refused(&e);
        assert_eq!(e.execution.committed, pre);
        assert!(e.execution.committed_logs.is_empty());
        assert_eq!(e.execution.writes.len(), 1);
        assert_eq!(e.execution.logs.len(), 1);
        assert!(matches!(&e.execution.exit,Exit::HarnessFailure(s) if s.contains("stack underflow")));
    }
    for result in [ScriptedResult::Return(vec![1, 2, 3]), ScriptedResult::Revert(vec![1, 2, 3])] {
        let mut c = context();
        let mut code = vec![];
        call(&mut code, &mut c, 9.into(), &[], U256::MAX, U256::MAX, 0, result.clone());
        push(&mut code, 0.into());
        code.push(0x52);
        ret(&mut code, 0, 32);
        let expected = if matches!(result, ScriptedResult::Return(_)) { 1 } else { 0 };
        assert_eq!(returned(&run(&code, &c)), word(expected.into()));
        for (offset, ok) in [(3, true), (4, false)] {
            let mut code = vec![];
            let mut c = context();
            call(&mut code, &mut c, 9.into(), &[], U256::MAX, U256::MAX, 0, result.clone());
            code.push(0x50);
            push(&mut code, 0.into());
            push(&mut code, offset.into());
            push(&mut code, U256::MAX);
            code.push(0x3e);
            code.push(0x00);
            let e = run(&code, &c);
            if ok {
                assert!(returned(&e).is_empty());
            } else {
                refused(&e);
            }
        }
    }
}
