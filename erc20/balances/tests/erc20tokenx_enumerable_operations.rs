#![cfg(not(target_arch = "wasm32"))]
//! Exact compiled role transcripts translated to synthetic Extended storage.
//! No deployed authorization, initialization or producer visibility is inferred.
use erc20_balances::{hash, layout, project};
use serde_json::{json, Value};
use std::{path::Path, sync::OnceLock};
use substreams_ethereum::pb::eth::v2 as eth;
type Word = [u8; 32];
const ZERO: Word = [0; 32];
const GRANT: &str = "grantRole(bytes32,address)";
const REVOKE: &str = "revokeRole(bytes32,address)";
const RENOUNCE: &str = "renounceRole(bytes32,address)";
fn n(v: u64) -> Word {
    let mut out = ZERO;
    out[24..].copy_from_slice(&v.to_be_bytes());
    out
}
fn bytes(s: &str) -> Vec<u8> {
    hex::decode(s.trim_start_matches("0x")).unwrap()
}
fn word(v: &Value) -> Word {
    bytes(v.as_str().unwrap()).try_into().unwrap()
}
fn hex(w: Word) -> String {
    format!("0x{}", hex::encode(w))
}
fn pair(a: Word, b: Word) -> Vec<u8> {
    [a.as_slice(), b.as_slice()].concat()
}
fn leaf(a: Word, b: Word) -> Word {
    hash(&pair(a, b))
}
fn plus(a: Word, b: Word) -> Word {
    let mut out = ZERO;
    let mut carry = 0u16;
    for i in (0..32).rev() {
        carry += a[i] as u16 + b[i] as u16;
        out[i] = carry as u8;
        carry >>= 8;
    }
    out
}
fn minus(a: Word, b: Word) -> Word {
    plus(plus(a, b.map(|x| !x)), n(1))
}
fn records() -> &'static [Value] {
    static RECORDS: OnceLock<Vec<Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        // Phase A must be merged before this source-derived matrix runs. A getter
        // sharing an operation name cannot substitute for the signature below.
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/evidence/erc20tokenx-operation-proof-20260929-transcripts.json");
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    })
}
fn case(name: &str, signature: &str) -> &'static Value {
    let matches: Vec<_> = records()
        .iter()
        .filter(|v| v["name"] == name && v["signature"] == signature && v["code_kind"] == "deployedBytecode")
        .collect();
    assert_eq!(matches.len(), 1, "unique operation {name}/{signature}");
    matches[0]
}
fn hint(call: &mut eth::Call, raw: Vec<u8>) {
    call.keccak_preimages.insert(hex::encode(hash(&raw)), hex::encode(raw));
}
struct Profile(Value);
impl Profile {
    fn all() -> Vec<Self> {
        serde_json::from_str::<Vec<Value>>(include_str!("fixtures/erc20tokenx-enumerable-candidate/layouts.json"))
            .unwrap()
            .into_iter()
            .map(Self)
            .collect()
    }
    fn account(&self) -> Vec<u8> {
        bytes(self.0["contract"].as_str().unwrap())
    }
    fn layouts(&self) -> Vec<layout::VerifiedLayout> {
        layout::parse(&json!([self.0]).to_string()).unwrap()
    }
    fn row(&self, key: Word, old: Word, new: Word, ordinal: u64) -> eth::StorageChange {
        eth::StorageChange {
            address: self.account(),
            key: key.to_vec(),
            old_value: old.to_vec(),
            new_value: new.to_vec(),
            ordinal,
        }
    }
    fn call(&self, name: &str, sig: &str, subset: usize) -> eth::Call {
        let c = case(name, sig);
        assert_eq!(c["execution"]["exit"]["kind"], "return", "{name}");
        let mut call = eth::Call {
            address: self.account(),
            begin_ordinal: 1,
            end_ordinal: 10000,
            ..Default::default()
        };
        for k in c["execution"]["keccaks"].as_array().unwrap() {
            let raw = bytes(k["input"].as_str().unwrap());
            assert_eq!(hex(hash(&raw)), k["output"]);
            hint(&mut call, raw);
        }
        let mut equality = 0;
        for w in c["execution"]["writes"].as_array().unwrap() {
            if w["old"] == w["new"] {
                let keep = subset & (1 << equality) != 0;
                equality += 1;
                if !keep {
                    continue;
                }
            }
            call.storage_changes
                .push(self.row(word(&w["key"]), word(&w["old"]), word(&w["new"]), w["step"].as_u64().unwrap() + 10));
        }
        call
    }
    fn saved(&self, name: &str, sig: &str) -> eth::Call {
        self.call(name, sig, usize::MAX)
    }
    fn block(&self, mut calls: Vec<eth::Call>) -> eth::Block {
        for (i, call) in calls.iter_mut().enumerate() {
            let shift = i as u64 * 10000;
            call.begin_ordinal += shift;
            call.end_ordinal += shift;
            for w in &mut call.storage_changes {
                w.ordinal += shift;
            }
        }
        // Every negative carries an independently valid balance update: an
        // incomplete role witness must refuse the entire projection.
        hint(&mut calls[0], pair(n(44), ZERO));
        calls[0].storage_changes.push(self.row(leaf(n(44), ZERO), ZERO, n(9), 9999));
        eth::Block {
            ver: 5,
            number: 122288046,
            hash: vec![7; 32],
            header: Some(eth::BlockHeader {
                number: 122288046,
                parent_hash: vec![6; 32],
                state_root: vec![8; 32],
                ..Default::default()
            }),
            detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
            transaction_traces: vec![eth::TransactionTrace {
                status: eth::TransactionTraceStatus::Succeeded as i32,
                begin_ordinal: 1,
                end_ordinal: calls.len() as u64 * 10000,
                calls,
                ..Default::default()
            }],
            ..Default::default()
        }
    }
    fn ok(&self, b: &eth::Block) {
        let rows = project(b, &self.layouts()).unwrap().balances;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].amount, "9");
        assert_eq!(rows[0].contract, Some(self.account()));
    }
    fn bad(&self, b: &eth::Block) {
        assert!(project(b, &self.layouts()).is_err(), "malformed role admitted for {}", self.0["contract"]);
    }
}
#[test]
fn tokenx_candidate_inverts_real_operation_refusal_and_fabricated_nested_permission() {
    let baseline: Vec<Value> = serde_json::from_str(include_str!("fixtures/bsc-refined450-layouts.json")).unwrap();
    for p in Profile::all() {
        let original = baseline.iter().find(|v| v["contract"] == p.0["contract"]).unwrap();
        let original_layout = layout::parse(&json!([original]).to_string()).unwrap();
        let real = p.block(vec![p.saved("role_add_empty", GRANT)]);
        assert!(
            project(&real, &original_layout).is_err(),
            "historical broad rule does not admit complete array operation"
        );
        p.ok(&real);
        let role = hash(b"TEST_ROLE");
        let r = leaf(role, n(8));
        let mut fabricated = eth::Call {
            address: p.account(),
            begin_ordinal: 1,
            end_ordinal: 10000,
            ..Default::default()
        };
        hint(&mut fabricated, pair(role, n(8)));
        hint(&mut fabricated, pair(n(77), r));
        fabricated.storage_changes.push(p.row(leaf(n(77), r), ZERO, n(1), 50));
        let b = p.block(vec![fabricated]);
        assert_eq!(project(&b, &original_layout).unwrap().balances.len(), 1, "old offset-zero recursive permission");
        p.bad(&b);
    }
}
#[test]
fn tokenx_exact_source_operations_accept_every_equal_stage_subset_at_each_profile() {
    let mut names: Vec<(String, &str)> = [
        ("role_add_empty", GRANT),
        ("role_add_nonempty", GRANT),
        ("role_add_zero_empty", GRANT),
        ("role_add_zero_nonempty", GRANT),
        ("role_duplicate", GRANT),
        ("role_duplicate_zero", GRANT),
        ("role_remove_first", REVOKE),
        ("role_remove_middle", REVOKE),
        ("role_remove_tail", REVOKE),
        ("role_remove_sole", REVOKE),
        ("role_remove_zero_sole", REVOKE),
        ("role_remove_zero_first", REVOKE),
        ("role_remove_zero_tail", REVOKE),
        ("role_move_zero_tail", REVOKE),
        ("role_absent_empty", REVOKE),
        ("role_absent_nonempty", REVOKE),
        ("renounce_self", RENOUNCE),
        ("renounce_tail", RENOUNCE),
        ("renounce_zero", RENOUNCE),
        ("renounce_absent", RENOUNCE),
        ("custom_admin_authorized", GRANT),
        ("max_admin_authorized", GRANT),
        ("max_admin_revoke", REVOKE),
        ("last_admin_renounce", RENOUNCE),
        ("repeat_last_admin_renounce", RENOUNCE),
    ]
    .into_iter()
    .map(|(n, s)| (n.to_owned(), s))
    .collect();
    names.extend((0..5).map(|i| (format!("role_identity_{i}"), GRANT)));
    for p in Profile::all() {
        for (name, sig) in &names {
            let c = case(name, sig);
            let equal = c["execution"]["writes"].as_array().unwrap().iter().filter(|w| w["old"] == w["new"]).count();
            for subset in 0..1 << equal {
                p.ok(&p.block(vec![p.call(name, sig, subset)]));
            }
        }
        let signatures = [GRANT, GRANT, GRANT, GRANT, REVOKE, RENOUNCE, REVOKE, GRANT, REVOKE];
        let sequence = signatures.iter().enumerate().map(|(i, s)| p.saved(&format!("sequence_{i}"), s)).collect();
        p.ok(&p.block(sequence));
    }
}
#[test]
fn tokenx_missing_extra_reordered_dirty_stages_and_preimages_refuse_atomically() {
    for p in Profile::all() {
        let c = p.saved("role_add_nonempty", GRANT);
        for i in 0..c.storage_changes.len() {
            let mut missing = c.clone();
            missing.storage_changes.remove(i);
            p.bad(&p.block(vec![missing]));
            for field in ["key", "old", "new"] {
                let mut dirty = c.clone();
                let w = &mut dirty.storage_changes[i];
                match field {
                    "key" => w.key = vec![1; 33],
                    "old" => w.old_value = vec![1; 33],
                    _ => w.new_value = vec![1; 33],
                }
                p.bad(&p.block(vec![dirty]));
            }
        }
        let mut order = c.clone();
        let a = order.storage_changes[0].ordinal;
        order.storage_changes[0].ordinal = order.storage_changes[1].ordinal;
        order.storage_changes[1].ordinal = a;
        p.bad(&p.block(vec![order]));
        let mut extra = c.clone();
        let mut w = extra.storage_changes[0].clone();
        w.old_value = w.new_value.clone();
        w.ordinal += 1;
        extra.storage_changes.insert(1, w);
        p.bad(&p.block(vec![extra]));
        let mut reused = c.clone();
        reused.storage_changes[1].ordinal = reused.storage_changes[0].ordinal;
        p.bad(&p.block(vec![reused]));
        let mut zero = c.clone();
        zero.storage_changes[0].ordinal = 0;
        p.bad(&p.block(vec![zero]));
        let mut absent = c.clone();
        absent.keccak_preimages.clear();
        p.bad(&p.block(vec![absent]));
        let mut corrupt = c.clone();
        let k = corrupt.keccak_preimages.keys().next().unwrap().clone();
        corrupt.keccak_preimages.insert(k, "ff".repeat(64));
        p.bad(&p.block(vec![corrupt]));
        // A real hash of a dirty address preimage is still not an address key.
        let role_head = leaf(hash(b"TEST_ROLE"), n(8));
        let pos = leaf(n(5), plus(role_head, n(1)));
        for mode in ["padding", "depth", "root", "order"] {
            let mut changed = c.clone();
            let mut raw = pair(n(5), plus(role_head, n(1)));
            match mode {
                "padding" => raw[0] = 1,
                "depth" => raw.extend_from_slice(&n(1)),
                "root" => raw[63] ^= 1,
                _ => raw = pair(plus(role_head, n(1)), n(5)),
            }
            changed.keccak_preimages.remove(&hex::encode(pos));
            let key = hash(&raw);
            hint(&mut changed, raw);
            changed.storage_changes.iter_mut().filter(|w| w.key == pos).for_each(|w| w.key = key.to_vec());
            p.bad(&p.block(vec![changed]));
        }
    }
}
#[test]
fn tokenx_admin_anchor_and_standalone_noops_are_not_new_permissions() {
    for p in Profile::all() {
        let role = hash(b"TEST_ROLE");
        let r = leaf(role, n(8));
        for offset in [1, 2] {
            for equal in [false, true] {
                let mut c = eth::Call {
                    address: p.account(),
                    begin_ordinal: 1,
                    end_ordinal: 10000,
                    ..Default::default()
                };
                hint(&mut c, pair(role, n(8)));
                c.storage_changes.push(p.row(plus(r, n(offset)), if equal { n(1) } else { ZERO }, n(1), 50));
                p.bad(&p.block(vec![c.clone()]));
                if equal {
                    c.keccak_preimages.clear();
                    p.ok(&p.block(vec![c]));
                }
            }
        }
        let mut c = p.saved("role_add_zero_empty", GRANT);
        c.storage_changes.retain(|w| w.old_value == w.new_value);
        // With no changing length witness, this equal array location is not an
        // admitted operation namespace. Ordinary unknown no-op policy ignores
        // it; do not import coupled-mode fragment rules into legacy semantics.
        p.ok(&p.block(vec![c.clone()]));
        c.storage_changes = vec![p.row(r, ZERO, ZERO, 50)];
        p.bad(&p.block(vec![c.clone()])); // Recognized standalone length no-op.
        c.storage_changes = vec![p.row(leaf(ZERO, plus(r, n(1))), n(1), n(1), 50)];
        p.bad(&p.block(vec![c])); // Recognized standalone position no-op.
        p.ok(&p.block(vec![p.saved("role_duplicate", GRANT)]));
    }
}
#[test]
fn tokenx_legacy_versions_boundaries_and_frame_barriers_are_preserved() {
    for p in Profile::all() {
        for version in [3, 4, 5] {
            let mut b = p.block(vec![p.saved("role_add_empty", GRANT)]);
            b.ver = version;
            p.ok(&b);
        }
        let mut legacy = p.block(vec![p.saved("role_add_empty", GRANT)]);
        legacy.ver = 3;
        legacy.transaction_traces[0].calls[0].begin_ordinal = 0;
        p.ok(&legacy);
        legacy.transaction_traces[0].begin_ordinal = 0;
        p.bad(&legacy);
        for version in [4, 5] {
            let mut b = p.block(vec![p.saved("role_add_empty", GRANT)]);
            b.ver = version;
            b.transaction_traces[0].calls[0].begin_ordinal = 0;
            p.bad(&b);
        }
        let c = p.saved("role_add_empty", GRANT);
        let mut first = c.clone();
        let tail = first.storage_changes.split_off(1);
        let mut second = c.clone();
        second.storage_changes = tail;
        p.bad(&p.block(vec![first, second]));
        let mut account = c.clone();
        account.storage_changes[1].address = vec![9; 20];
        p.bad(&p.block(vec![account]));
        let mut b = p.block(vec![c.clone()]);
        let middle = (c.storage_changes[0].ordinal + c.storage_changes[1].ordinal) / 2;
        b.transaction_traces[0].calls[0].storage_changes.push(eth::StorageChange {
            address: vec![9; 20],
            key: n(88).to_vec(),
            old_value: ZERO.to_vec(),
            new_value: ZERO.to_vec(),
            ordinal: middle,
        });
        p.bad(&b);
        let mut b = p.block(vec![c]);
        b.transaction_traces[0].calls.push(eth::Call {
            address: vec![9; 20],
            index: 1,
            parent_index: 0,
            depth: 1,
            begin_ordinal: middle,
            end_ordinal: middle + 1,
            state_reverted: true,
            ..Default::default()
        });
        p.bad(&b);
    }
}
#[test]
fn tokenx_malformed_source_successes_are_not_coherent_admission() {
    for p in Profile::all() {
        for (name, sig) in [
            ("malformed_max_length_push", GRANT),
            ("malformed_index_points_to_other_member", REVOKE),
            ("malformed_dirty_tail_word", REVOKE),
        ] {
            p.bad(&p.block(vec![p.saved(name, sig)]));
        }
        // An unobserved corrupted initial state cannot be disproved by no stores.
        p.ok(&p.block(vec![p.saved("malformed_duplicate_ignores_length", GRANT)]));
        let mut b = p.block(vec![p.saved("role_add_empty", GRANT), p.saved("role_add_empty", GRANT)]);
        p.bad(&b);
        b.transaction_traces[0].calls[1].state_reverted = true;
        p.ok(&b);
    }
}

// Deliberately mathematical controls for modular physical aliases and full-word
// lengths absent from coherent source fixtures; not extra runtime observations.
fn synthetic_add(p: &Profile, role: Word, length: Word, member: Word) -> eth::Call {
    let head = leaf(role, n(8));
    let position = leaf(member, plus(head, n(1)));
    let mut c = eth::Call {
        address: p.account(),
        begin_ordinal: 1,
        end_ordinal: 10000,
        ..Default::default()
    };
    hint(&mut c, pair(role, n(8)));
    hint(&mut c, pair(member, plus(head, n(1))));
    c.storage_changes = vec![
        p.row(head, length, plus(length, n(1)), 50),
        p.row(plus(hash(&head), length), ZERO, member, 60),
        p.row(position, ZERO, plus(length, n(1)), 70),
    ];
    c
}
#[test]
fn tokenx_modular_aliases_and_omitted_zero_continuity_never_grant_extra_permission() {
    for p in Profile::all() {
        let role = n(731);
        let head = leaf(role, n(8));
        let array = hash(&head);
        for target in [n(3), leaf(n(44), ZERO), leaf(n(17), plus(head, n(1))), head, plus(head, n(1)), plus(head, n(2))] {
            p.bad(&p.block(vec![synthetic_add(&p, role, minus(target, array), n(17))]));
        }
        // A wrapped, otherwise unprotected physical word remains a valid
        // logical array location; no usize/u64 truncation was introduced.
        p.ok(&p.block(vec![synthetic_add(&p, role, minus(n(999), array), n(17))]));
        let add = p.saved("role_add_empty", GRANT);
        let removed_zero = p.call("role_remove_zero_sole", REVOKE, 0);
        p.bad(&p.block(vec![add, removed_zero]));
        let added_zero = p.call("role_add_zero_empty", GRANT, 0);
        p.bad(&p.block(vec![added_zero, p.saved("role_remove_sole", REVOKE)]));
        let second_role = n(732);
        let second_head = leaf(second_role, n(8));
        let one = synthetic_add(&p, role, minus(second_head, array), n(17));
        let two = synthetic_add(&p, second_role, ZERO, n(18));
        for calls in [vec![one.clone(), two.clone()], vec![two.clone(), one.clone()]] {
            p.bad(&p.block(calls));
        }
    }
}
#[test]
fn tokenx_roles_transactions_and_storage_accounts_cannot_supply_each_others_stages() {
    for p in Profile::all() {
        let a = p.saved("role_add_empty", GRANT);
        let b = p.saved("role_identity_1", GRANT);
        p.ok(&p.block(vec![a.clone(), b.clone()]));
        let mut joined = a.clone();
        joined.storage_changes[1..].clone_from_slice(&b.storage_changes[1..]);
        joined.keccak_preimages.extend(b.keccak_preimages.clone());
        p.bad(&p.block(vec![joined]));
        let mut left = a.clone();
        let mut right = a;
        right.storage_changes = left.storage_changes.split_off(1);
        let mut block = p.block(vec![left, right]);
        let second = block.transaction_traces[0].calls.pop().unwrap();
        block.transaction_traces[0].end_ordinal = 10000;
        block.transaction_traces.push(eth::TransactionTrace {
            index: 1,
            status: eth::TransactionTraceStatus::Succeeded as i32,
            begin_ordinal: 10001,
            end_ordinal: 20000,
            calls: vec![second],
            ..Default::default()
        });
        p.bad(&block);
    }
    let profiles = Profile::all();
    let b = profiles[0].block(vec![profiles[0].saved("role_add_empty", GRANT), profiles[1].saved("role_add_empty", GRANT)]);
    let layouts = layout::parse(&json!([profiles[0].0, profiles[1].0]).to_string()).unwrap();
    assert_eq!(project(&b, &layouts).unwrap().balances.len(), 1);
}
#[test]
fn tokenx_unrelated_metadata_runtime_changes_and_unqualified_creation_remain_exact() {
    for p in Profile::all() {
        let mut c = p.saved("role_add_empty", GRANT);
        for (i, slot) in [2, 3, 4, 5, 7, 9, 10, 11, 12].into_iter().enumerate() {
            c.storage_changes.push(p.row(n(slot), n(1), n(2), 8000 + i as u64));
        }
        let allow = leaf(n(44), n(1));
        hint(&mut c, pair(n(44), n(1)));
        hint(&mut c, pair(n(55), allow));
        c.storage_changes.push(p.row(leaf(n(55), allow), ZERO, n(8), 8100));
        hint(&mut c, pair(n(44), n(6)));
        c.storage_changes.push(p.row(leaf(n(44), n(6)), ZERO, n(1), 8200));
        p.ok(&p.block(vec![c]));
        let mut changed = p.block(vec![p.saved("role_add_empty", GRANT)]);
        changed.transaction_traces[0].calls[0].code_changes = vec![
            eth::CodeChange {
                address: p.account(),
                old_hash: word(&p.0["code_hash"]).to_vec(),
                new_hash: hash(&[1]).to_vec(),
                new_code: vec![1],
                ordinal: 8500,
                ..Default::default()
            },
            eth::CodeChange {
                address: p.account(),
                old_hash: hash(&[1]).to_vec(),
                new_hash: word(&p.0["code_hash"]).to_vec(),
                ordinal: 8600,
                ..Default::default()
            },
        ];
        p.bad(&changed);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/erc20tokenx-operation-proof/ori-capture.json");
        let capture: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let runtime = bytes(capture["runtimeBytecode"]["onchainBytecode"].as_str().unwrap());
        assert_eq!(hex(hash(&runtime)), p.0["code_hash"]);
        let mut creation = p.block(vec![p.saved("role_add_empty", GRANT)]);
        let call = &mut creation.transaction_traces[0].calls[0];
        call.call_type = eth::CallType::Create as i32;
        call.code_changes.push(eth::CodeChange {
            address: p.account(),
            old_hash: hash(&[]).to_vec(),
            new_hash: hash(&runtime).to_vec(),
            new_code: runtime,
            ordinal: 8500,
            ..Default::default()
        });
        p.bad(&creation);
    }
}

#[test]
fn tokenx_critical_hints_normalized_words_dirty_members_and_reverted_prefixes_are_exact() {
    for p in Profile::all() {
        let head = leaf(hash(b"TEST_ROLE"), n(8));
        let position = leaf(n(5), plus(head, n(1)));
        for key in [head, position] {
            let mut c = p.saved("role_add_nonempty", GRANT);
            assert!(c.keccak_preimages.remove(&hex::encode(key)).is_some());
            p.bad(&p.block(vec![c]));
        }
        // The explicit array-base hint is unnecessary; head proof derives it.
        let mut c = p.saved("role_add_nonempty", GRANT);
        c.keccak_preimages.remove(&hex::encode(hash(&head)));
        p.ok(&p.block(vec![c]));
        let mut c = p.saved("role_add_nonempty", GRANT);
        for w in &mut c.storage_changes {
            for value in [&mut w.key, &mut w.old_value, &mut w.new_value] {
                let first = value.iter().position(|b| *b != 0).unwrap_or(value.len());
                value.drain(..first);
            }
        }
        p.ok(&p.block(vec![c])); // Legacy normalization, not coupled exact-key policy.
        let mut c = p.saved("role_add_nonempty", GRANT);
        c.storage_changes[1].new_value[0] = 1;
        p.bad(&p.block(vec![c]));
        let mut malformed = p.saved("role_add_nonempty", GRANT);
        malformed.storage_changes.remove(1);
        let mut b = p.block(vec![
            eth::Call {
                address: p.account(),
                begin_ordinal: 1,
                end_ordinal: 10000,
                ..Default::default()
            },
            malformed,
        ]);
        p.bad(&b);
        b.transaction_traces[0].calls[1].state_reverted = true;
        p.ok(&b);
        b.transaction_traces[0].calls[1].state_reverted = false;
        let call = b.transaction_traces[0].calls.pop().unwrap();
        b.transaction_traces[0].end_ordinal = 10000;
        b.transaction_traces.push(eth::TransactionTrace {
            index: 1,
            status: eth::TransactionTraceStatus::Failed as i32,
            begin_ordinal: 10001,
            end_ordinal: 20000,
            calls: vec![call],
            ..Default::default()
        });
        p.ok(&b);
        for sig in [GRANT, REVOKE, RENOUNCE] {
            let name = match sig {
                GRANT => "wrong_grant_admin",
                REVOKE => "wrong_revoke_admin",
                _ => "wrong_renounce_account",
            };
            let c = case(name, sig);
            assert_eq!(c["execution"]["exit"]["kind"], "revert");
            assert!(c["execution"]["writes"].as_array().unwrap().is_empty());
        }
    }
}
#[test]
fn tokenx_legacy_schema_rejects_membership_null_and_overlapping_permissions() {
    for p in Profile::all() {
        for value in [Value::Null, json!(hex(n(8)))] {
            let mut c = p.0.clone();
            c["enumerable_address_sets"][0]["membership_root"] = value;
            assert!(layout::parse(&json!([c]).to_string()).is_err());
        }
        for (field, value) in [
            ("balance_slot", json!(hex(n(8)))),
            ("other_slots", json!([hex(n(8))])),
            ("other_mapping_slots", json!([hex(n(8))])),
            ("other_mapping_words", json!({hex(n(8)):3})),
            ("other_mapping_paths", json!([{"root":hex(n(8)),"key_types":["bytes32"],"offset":2,"words":1}])),
            ("address_lists", json!([hex(n(8))])),
            ("voting_checkpoints", json!({"clock":"block_number","slots":[hex(n(8))]})),
            ("zero_balance", json!({"value":hex(n(1)),"storage_slot":hex(n(8))})),
            ("balance_divisor", json!({"value":hex(n(2)),"storage_slot":hex(n(8))})),
        ] {
            let mut c = p.0.clone();
            c[field] = value;
            assert!(layout::parse(&json!([c]).to_string()).is_err(), "{field}");
        }
        let mut c = p.0.clone();
        let rule = c["enumerable_address_sets"][0].clone();
        c["enumerable_address_sets"].as_array_mut().unwrap().push(rule);
        assert!(layout::parse(&json!([c]).to_string()).is_err());
    }
}
