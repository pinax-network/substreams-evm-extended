use super::vm::*;
use primitive_types::U256;
fn run(hex: &str) -> Execution {
    execute(&hex::decode(hex).unwrap(), &[], 0x11.into(), 0x22.into(), &State::new())
}
#[test]
fn explicit_self_code_size_distinguishes_constructor_implementation_and_proxy_context() {
    let code = hex::decode("303b5f5260205ff3").unwrap();
    for size in [0, 15308, 183] {
        let e = execute_with_self_code_size(&code, &[], 1.into(), 2.into(), &State::new(), Some(size));
        assert_eq!(returned(e), word(size.into()));
    }
    assert!(matches!(execute(&code, &[], 1.into(), 2.into(), &State::new()).exit, Exit::HarnessFailure(_)));
    // EXTCODESIZE interprets the operand as a 160-bit address.
    let mut high = vec![0x7f];
    high.extend(word((U256::one() << 255) + U256::from(2)));
    high.extend(hex::decode("3b5f5260205ff3").unwrap());
    assert_eq!(
        returned(execute_with_self_code_size(&high, &[], 1.into(), 2.into(), &State::new(), Some(183))),
        word(183.into())
    );
}
#[test]
fn explicit_self_context_never_invents_external_accounts_and_preserves_failure_rollback() {
    let prestate = State::from([(7.into(), 9.into())]);
    for opcode in [0x3b, 0x3c, 0x46, 0xf1, 0xf4, 0xfa] {
        // Attempt a storage write before an unknown account query or unsupported opcode.
        let mut code = hex::decode("60016007556003").unwrap();
        code.push(opcode);
        let e = execute_with_self_code_size(&code, &[], 1.into(), 2.into(), &prestate, Some(183));
        assert!(matches!(e.exit, Exit::HarnessFailure(_)), "opcode {opcode:02x}");
        assert_eq!(e.writes.len(), 1);
        assert_eq!(e.committed, prestate);
        assert!(e.committed_logs.is_empty());
    }
}
fn returned(e: Execution) -> Vec<u8> {
    match e.exit {
        Exit::Return(data) => data,
        other => panic!("{other:?}"),
    }
}
fn binary(op: u8, a: U256, b: U256) -> U256 {
    let mut code = vec![0x7f];
    code.extend(word(b));
    code.push(0x7f);
    code.extend(word(a));
    code.push(op);
    code.extend(hex::decode("5f5260205ff3").unwrap());
    U256::from_big_endian(&returned(execute(&code, &[], 0.into(), 0.into(), &State::new())))
}
#[test]
fn ptoken_vm_word_arithmetic_order_wrapping_and_signed_boundaries() {
    let max = U256::max_value();
    for (op, a, b, want) in [
        (0x01, max, 1.into(), 0.into()),
        (0x02, max, 2.into(), max - 1),
        (0x03, 1.into(), 2.into(), max),
        (0x04, 7.into(), 2.into(), 3.into()),
        (0x04, 7.into(), 0.into(), 0.into()),
        (0x06, 7.into(), 2.into(), 1.into()),
        (0x0a, 2.into(), 256.into(), 0.into()),
        (0x10, 1.into(), 2.into(), 1.into()),
        (0x11, 2.into(), 1.into(), 1.into()),
        (0x12, max, 0.into(), 1.into()),
        (0x13, 0.into(), max, 1.into()),
        (0x0b, 0.into(), 128.into(), max - 127),
        (0x1b, 256.into(), 1.into(), 0.into()),
        (0x1c, 255.into(), max, 1.into()),
        (0x1d, 256.into(), max, max),
    ] {
        assert_eq!(binary(op, a, b), want, "opcode {op:02x}");
    }
}
#[test]
fn ptoken_vm_sparse_storage_reads_and_same_value_stores_preserve_full_words() {
    let max = U256::max_value();
    let mut code = vec![0x60, 7, 0x7f];
    code.extend(word(max));
    code.push(0x55);
    code.push(0x60);
    code.push(7);
    code.push(0x7f);
    code.extend(word(max));
    code.push(0x55);
    code.push(0x7f);
    code.extend(word(max));
    code.extend(hex::decode("545f5260205ff3").unwrap());
    let e = execute(&code, &[], 0.into(), 0.into(), &State::new());
    assert_eq!(e.writes.len(), 2);
    assert_eq!(e.writes[0].old, U256::zero());
    assert_eq!(e.writes[1].old, e.writes[1].new);
    assert_eq!(e.committed[&max], 7.into());
    assert_eq!(returned(e), word(7.into()));
    assert_eq!(returned(run("6050545f5260205ff3")), word(0.into()));
}
#[test]
fn ptoken_vm_caller_address_calldata_and_hash_witnesses_are_actual_inputs() {
    assert_eq!(returned(run("335f523060205260405ff3")), [word(0x11.into()), word(0x22.into())].concat());
    let e = run("5f5f205f5260205ff3");
    assert!(e.keccaks[0].input.is_empty());
    assert_eq!(
        hex::encode(word(e.keccaks[0].output)),
        "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
    );
    assert_eq!(returned(e).len(), 32);
    let e = execute(
        &hex::decode("602060005f3760205f205f5260205ff3").unwrap(),
        &[1, 2, 3],
        0.into(),
        0.into(),
        &State::new(),
    );
    assert_eq!(&e.keccaks[0].input[..4], &[1, 2, 3, 0]);
    assert_eq!(e.keccaks[0].input.len(), 32);
    let e = execute(&hex::decode("6001355f5260205ff3").unwrap(), &[9, 8, 7], 0.into(), 0.into(), &State::new());
    assert_eq!(&returned(e)[..3], &[8, 7, 0]);
}
#[test]
fn ptoken_vm_mcopy_is_overlap_safe_expands_with_zero_and_handles_zero_length() {
    // MSTORE right-aligns 01 02 03 04 at offsets 28..31. Overlapping MCOPY
    // must read a snapshot rather than repeatedly copying its newly written byte.
    assert_eq!(returned(run("63010203045f526004601c601d5e6005601cf3")), vec![1, 1, 2, 3, 4]);
    assert_eq!(returned(run("63010203045f526004601c601b5e6005601bf3")), vec![1, 2, 3, 4, 4]);
    assert_eq!(returned(run("600460805f5e60045ff3")), vec![0; 4]);
    let mut code = vec![0x5f, 0x7f];
    code.extend(word(U256::max_value()));
    code.push(0x7f);
    code.extend(word(U256::max_value()));
    code.extend([0x5e, 0x59, 0x5f, 0x52, 0x60, 0x20, 0x5f, 0xf3]);
    assert_eq!(returned(execute(&code, &[], 0.into(), 0.into(), &State::new())), word(0.into()));
}
#[test]
fn ptoken_vm_return_revert_invalid_and_harness_failure_keep_atomic_state_and_logs() {
    let prefix = hex::decode("600160025560045f52600360205fa1").unwrap();
    let prestate = State::from([(2.into(), 9.into())]);
    for (suffix, expected) in [
        ("5f5ff3", Exit::Return(vec![])),
        ("5f5ffd", Exit::Revert(vec![])),
        ("fe", Exit::Invalid),
        ("ff", Exit::HarnessFailure("unsupported opcode 0xff at pc 14".into())),
    ] {
        let mut code = prefix.clone();
        code.extend(hex::decode(suffix).unwrap());
        let e = execute(&code, &[], 0.into(), 0.into(), &prestate);
        if matches!(expected, Exit::HarnessFailure(_)) {
            assert!(matches!(e.exit, Exit::HarnessFailure(_)))
        } else {
            assert_eq!(e.exit, expected)
        }
        assert_eq!(e.writes.len(), 1);
        assert_eq!(e.writes[0].old, 9.into());
        assert_eq!(e.logs.len(), 1);
        assert_eq!(e.logs[0].topics, vec![3.into()]);
        assert_eq!(e.logs[0].data, word(4.into()));
        if matches!(expected, Exit::Return(_)) {
            assert_eq!(e.committed[&2.into()], 1.into());
            assert_eq!(e.committed_logs, e.logs)
        } else {
            assert_eq!(e.committed, prestate);
            assert!(e.committed_logs.is_empty())
        }
    }
    assert_eq!(run("5f5ffd").exit, Exit::Revert(vec![]));
    assert_eq!(run("fe").exit, Exit::Invalid);
}
#[test]
fn ptoken_vm_invalid_jumps_stack_and_resource_limits_are_harness_failures() {
    for code in ["600456605b00", "50", "80", "90", "6210000151", "5c"] {
        assert!(matches!(run(code).exit, Exit::HarnessFailure(_)), "{code}");
    }
    assert!(matches!(run("5b5f56").exit, Exit::HarnessFailure(_)));
    assert!(matches!(
        execute(&vec![0x5f; 1025], &[], 0.into(), 0.into(), &State::new()).exit,
        Exit::HarnessFailure(_)
    ));
    // EVM zero-pads an incomplete PUSH, then falling off code stops.
    assert_eq!(run("7f01").exit, Exit::Return(vec![]));
    assert_eq!(run("6003565b00").exit, Exit::Return(vec![]));
}

#[test]
fn ptoken_vm_zero_sized_memory_ignores_max_offset_and_witness_limit_rolls_back() {
    for op in [0x20, 0xa0, 0xf3, 0xfd] {
        let mut code = vec![0x5f, 0x7f];
        code.extend(word(U256::max_value()));
        code.push(op);
        let e = execute(&code, &[], 0.into(), 0.into(), &State::new());
        assert_eq!(e.exit, if op == 0xfd { Exit::Revert(vec![]) } else { Exit::Return(vec![]) });
        if op == 0x20 {
            assert!(e.keccaks[0].input.is_empty());
        }
        if op == 0xa0 {
            assert!(e.logs[0].data.is_empty());
        }
    }
    // A successful write and empty log precede three 32-byte hashes. The third
    // exceeds the test's 64-byte aggregate cap; prior evidence is retained.
    let code = hex::decode("60016002555f5fa060205f205060205f205060205f205000").unwrap();
    let prestate = State::from([(2.into(), 9.into())]);
    let e = small_witness_execution(&code, &prestate);
    assert_eq!(e.exit, Exit::HarnessFailure("witness byte bound exceeded".into()));
    assert_eq!(e.keccaks.len(), 2);
    assert_eq!(e.writes.len(), 1);
    assert_eq!(e.logs.len(), 1);
    assert_eq!(e.committed, prestate);
    assert!(e.committed_logs.is_empty());
}
