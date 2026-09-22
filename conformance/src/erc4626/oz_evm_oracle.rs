//! Independent execution of solc 0.8.20's pinned OZ ERC4626/Math bytecode.
//! This bounded, test-only interpreter supports only the arithmetic/getter
//! path opcodes. Unsupported opcodes panic, never masquerade as a revert.
//! It is not a network client, a general EVM, or deployment qualification.
use super::*;
use num_traits::ToPrimitive;

const RUNTIME: &str = include_str!("../../fixtures/oz-v5-oracle.bin-runtime");

fn decode(hex: &str) -> Vec<u8> {
    let hex = hex.trim().as_bytes();
    assert_eq!(hex.len() % 2, 0);
    hex.chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn word(value: &BigUint) -> [u8; 32] {
    let bytes = value.to_bytes_be();
    assert!(bytes.len() <= 32);
    let mut result = [0; 32];
    result[32 - bytes.len()..].copy_from_slice(&bytes);
    result
}

/// Execute a pure getter with controlled storage. All arithmetic opcodes
/// wrap at 256 bits; MULMOD retains its unbounded intermediate as in EVM.
fn execute(code: &[u8], calldata: &[u8], storage: &[BigUint]) -> std::result::Result<Vec<u8>, Vec<u8>> {
    let modulus = BigUint::one() << 256u32;
    let mask = &modulus - BigUint::one();
    let mut stack: Vec<BigUint> = Vec::new();
    let mut memory = vec![0u8; 65_536];
    let mut pc = 0;
    // Build valid jump destinations excluding bytes contained in PUSH data.
    let mut destinations = vec![false; code.len()];
    while pc < code.len() {
        let op = code[pc];
        destinations[pc] = op == 0x5b;
        pc += 1 + if (0x60..=0x7f).contains(&op) { usize::from(op - 0x5f) } else { 0 };
    }
    pc = 0;
    for _ in 0..100_000 {
        let op = code[pc];
        pc += 1;
        match op {
            0x00 => return Ok(Vec::new()),
            0x01..=0x04 | 0x06 | 0x0a | 0x10..=0x12 | 0x14 | 0x16..=0x18 | 0x1b..=0x1c => {
                let a = stack.pop().unwrap();
                let b = stack.pop().unwrap();
                let value = match op {
                    0x01 => a + b,
                    0x02 => a * b,
                    0x03 => a + &modulus - b,
                    0x04 => {
                        if b.is_zero() {
                            BigUint::zero()
                        } else {
                            a / b
                        }
                    }
                    0x06 => {
                        if b.is_zero() {
                            BigUint::zero()
                        } else {
                            a % b
                        }
                    }
                    0x0a => a.modpow(&b, &modulus),
                    0x10 => BigUint::from(u8::from(a < b)),
                    0x11 => BigUint::from(u8::from(a > b)),
                    0x12 => {
                        let a_negative = a.bit(255);
                        let b_negative = b.bit(255);
                        BigUint::from(u8::from(if a_negative == b_negative { a < b } else { a_negative }))
                    }
                    0x14 => BigUint::from(u8::from(a == b)),
                    0x16 => a & b,
                    0x17 => a | b,
                    0x18 => a ^ b,
                    0x1b | 0x1c => {
                        if a >= BigUint::from(256u32) {
                            BigUint::zero()
                        } else if op == 0x1b {
                            b << a.to_usize().unwrap()
                        } else {
                            b >> a.to_usize().unwrap()
                        }
                    }
                    _ => unreachable!(),
                };
                stack.push(value & &mask);
            }
            0x08 | 0x09 => {
                let a = stack.pop().unwrap();
                let b = stack.pop().unwrap();
                let m = stack.pop().unwrap();
                stack.push(if m.is_zero() {
                    BigUint::zero()
                } else if op == 0x08 {
                    (a + b) % m
                } else {
                    (a * b) % m
                });
            }
            0x15 => {
                let a = stack.pop().unwrap();
                stack.push(BigUint::from(u8::from(a.is_zero())));
            }
            0x19 => {
                let a = stack.pop().unwrap();
                stack.push(&mask ^ a);
            }
            0x34 => stack.push(BigUint::zero()), // CALLVALUE
            0x35 => {
                let offset = stack.pop().unwrap().to_usize().unwrap();
                let mut bytes = [0u8; 32];
                for (i, byte) in bytes.iter_mut().enumerate() {
                    *byte = calldata.get(offset + i).copied().unwrap_or(0);
                }
                stack.push(BigUint::from_bytes_be(&bytes));
            }
            0x36 => stack.push(BigUint::from(calldata.len())),
            0x50 => {
                stack.pop().unwrap();
            }
            0x51 => {
                let offset = stack.pop().unwrap().to_usize().unwrap();
                stack.push(BigUint::from_bytes_be(&memory[offset..offset + 32]));
            }
            0x52 => {
                let offset = stack.pop().unwrap().to_usize().unwrap();
                let value = stack.pop().unwrap();
                memory[offset..offset + 32].copy_from_slice(&word(&value));
            }
            0x54 => {
                let slot = stack.pop().unwrap().to_usize().unwrap();
                stack.push(storage.get(slot).expect("unbound storage in oracle path").clone());
            }
            0x56 | 0x57 => {
                let target = stack.pop().unwrap().to_usize().unwrap();
                if op == 0x56 || !stack.pop().unwrap().is_zero() {
                    assert!(destinations[target], "invalid jump");
                    pc = target;
                }
            }
            0x5b => {}
            0x60..=0x7f => {
                let width = usize::from(op - 0x5f);
                stack.push(BigUint::from_bytes_be(&code[pc..pc + width]));
                pc += width;
            }
            0x80..=0x8f => stack.push(stack[stack.len() - usize::from(op - 0x7f)].clone()),
            0x90..=0x9f => {
                let end = stack.len() - 1;
                stack.swap(end, end - usize::from(op - 0x8f));
            }
            0xf3 | 0xfd => {
                let offset = stack.pop().unwrap().to_usize().unwrap();
                let length = stack.pop().unwrap().to_usize().unwrap();
                let output = memory[offset..offset + length].to_vec();
                return if op == 0xf3 { Ok(output) } else { Err(output) };
            }
            _ => panic!("unsupported oracle opcode 0x{op:02x} at {}", pc - 1),
        }
        assert!(stack.len() <= 1024);
    }
    panic!("oracle step budget exceeded")
}

fn oracle(vault: &OzVirtualOffset, amount: &BigUint, selector: &str) -> std::result::Result<BigUint, Vec<u8>> {
    let mut calldata = decode(selector);
    calldata.extend(word(amount));
    execute(
        &decode(RUNTIME),
        &calldata,
        &[
            vault.total_assets.clone(),
            vault.total_supply.clone(),
            BigUint::from(vault.decimals_offset),
            amount.clone(),
        ],
    )
    .map(|bytes| {
        assert_eq!(bytes.len(), 32);
        BigUint::from_bytes_be(&bytes)
    })
}

#[test]
fn compiled_pinned_oz_getters_match_boundary_and_deterministic_generated_vectors() {
    assert_eq!(decode(RUNTIME).len(), 4_575, "pinned solc runtime size");
    let one = BigUint::one();
    let max = max_uint256();
    let pow100 = &one << 100u32;
    let pow200 = &one << 200u32;
    let mut cases = vec![
        (BigUint::zero(), BigUint::zero(), 0, BigUint::zero()),
        (BigUint::zero(), BigUint::zero(), 12, one.clone()),
        (BigUint::from(1000u32), BigUint::from(3u32), 0, one.clone()),
        (&pow100 - &one, &pow100 - &one, 0, pow200),
        (max.clone(), one.clone(), 0, BigUint::zero()),
        (BigUint::zero(), max.clone(), 0, one.clone()),
        (&max - BigUint::from(2u8), &max - BigUint::from(3u8), 0, &max - &one),
        (&max - BigUint::from(3u8), &max - BigUint::from(2u8), 0, &max - &one),
        (BigUint::zero(), BigUint::zero(), 77, one.clone()),
        (BigUint::zero(), BigUint::zero(), 78, BigUint::zero()),
        (BigUint::zero(), BigUint::zero(), 255, one.clone()),
        (&max - &one, BigUint::zero(), 0, max.clone()),
    ];
    // Independent deterministic inputs include near-zero and near-MAX totals,
    // unequal denominators, odd/even denominators and both mulDiv branches.
    let mut seed = 0x4626_2026_0921_u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for i in 0u32..96 {
        let a = BigUint::from(next());
        let b = BigUint::from(next());
        let amount = (BigUint::from(next()) << (i % 193)) & &max;
        cases.push((a.clone(), b.clone(), (i % 20) as u8, amount.clone()));
        cases.push((&max - a, &max - b, (i % 3) as u8, amount));
    }
    let mut successful = 0;
    let mut reverted = 0;
    for (assets, supply, offset, amount) in cases {
        let v = OzVirtualOffset {
            total_assets: assets,
            total_supply: supply,
            decimals_offset: offset,
        };
        for (selector, result) in [
            ("07a2d13a", v.convert_to_assets(&amount)),
            ("4cdad506", v.convert_to_assets(&amount)), // previewRedeem
            ("c6e6f592", v.convert_to_shares(&amount)),
            ("ef8b30f7", v.convert_to_shares(&amount)), // previewDeposit
            ("b3d7f6b9", v.preview_mint(&amount)),
            ("0a28a477", v.preview_withdraw(&amount)),
        ] {
            match (oracle(&v, &amount, selector), result) {
                (Ok(expected), Ok(actual)) => {
                    assert_eq!(actual, expected, "{selector} {v:?}, amount {amount}");
                    successful += 1;
                }
                (Err(revert), Err(Unknown::Invalid("uint256 overflow"))) => {
                    assert!(
                        revert == decode("227bc153") || revert == decode("4e487b710000000000000000000000000000000000000000000000000000000000000011"),
                        "unexpected revert: {revert:?}"
                    );
                    reverted += 1;
                }
                (expected, actual) => panic!("{selector} {v:?}, amount {amount}: EVM={expected:?}, Rust={actual:?}"),
            }
        }
    }
    assert!(successful > 500 && reverted > 10, "must exercise success and Solidity reverts");
}

#[test]
fn oracle_vm_arithmetic_uses_evm_word_semantics() {
    // SUB pops its left operand first; DIV/MOD follow the same stack order.
    assert_eq!(execute(&decode("600360050360005260206000f3"), &[], &[]).unwrap(), word(&BigUint::from(2u8)));
    assert_eq!(execute(&decode("600360080460005260206000f3"), &[], &[]).unwrap(), word(&BigUint::from(2u8)));
    assert_eq!(execute(&decode("600360080660005260206000f3"), &[], &[]).unwrap(), word(&BigUint::from(2u8)));
    // 0 - 1 wraps to MAX; MULMOD(MAX, MAX, MAX-1) = 1, not a truncated product.
    assert_eq!(execute(&decode("600160000360005260206000f3"), &[], &[]).unwrap(), word(&max_uint256()));
    let mut code = vec![0x7f];
    code.extend(word(&(max_uint256() - BigUint::one())));
    for _ in 0..2 {
        code.push(0x7f);
        code.extend(word(&max_uint256()));
    }
    code.extend(decode("0960005260206000f3"));
    assert_eq!(execute(&code, &[], &[]).unwrap(), word(&BigUint::one()));
    assert_eq!(
        execute(&decode("6001600a576007600d565b60085b60005260206000f3"), &[], &[]).unwrap(),
        word(&BigUint::from(8u8))
    );
    assert_eq!(
        execute(&decode("6000600a576007600d565b60085b60005260206000f3"), &[], &[]).unwrap(),
        word(&BigUint::from(7u8))
    );
}

#[test]
#[should_panic(expected = "unsupported oracle opcode")]
fn an_unsupported_opcode_is_a_harness_failure_not_a_solidity_revert() {
    let _ = execute(&[0xfe], &[], &[]);
}

#[test]
#[should_panic(expected = "invalid jump")]
fn jumping_inside_push_data_is_a_harness_failure() {
    let _ = execute(&decode("605b600156"), &[], &[]);
}
