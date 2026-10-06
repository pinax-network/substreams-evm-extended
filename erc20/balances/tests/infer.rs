//! Inference rules on synthetic blocks, block failures, the manifest shape and
//! captured block 122260950 against its saved same-block RPC values, also as a
//! firehose-tracer 5.5.0 producer would record it.
use buffa::Message;
use erc20_balances::run;
use substreams_ethereum::pb::eth::v2 as eth;
use tiny_keccak::{Hasher, Keccak};

fn hash(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0; 32];
    let mut hasher = Keccak::v256();
    hasher.update(bytes);
    hasher.finalize(&mut out);
    out
}

const TOKEN: [u8; 20] = [0x70; 20];
const IMPL: [u8; 20] = [0x71; 20];
const BEACON: [u8; 20] = [0x72; 20];
const FROM: [u8; 20] = [0xf0; 20];
const A: [u8; 20] = [0xa1; 20];
const B: [u8; 20] = [0xb2; 20];
const C: [u8; 20] = [0xc3; 20];
const D: [u8; 20] = [0xd4; 20];
const E: [u8; 20] = [0xe5; 20];
const FEE: [u8; 20] = [0xfe; 20];
const BASE: u8 = 3;
const TRANSFER: &str = "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";
const MINT: &str = "0f6798a560793a54c3bcfe86a93cde1e73087d944c0ea20544137d4121396885";
const DEPOSIT: &str = "e1fffcc4923d04b559f4d29a8bfc6cda04eb5b0d3c460751c2402c5c5cc9109c";
const WITHDRAWAL: &str = "7fcf532c15f0a6db0bd6d0e038bea71d30d808c7d98cb3bf7268a95bf5081b65";
const APPROVAL: &str = "8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925";
/// FiatToken v2.2 `Blacklisted(address indexed)`, a reference candidate shape.
const BLACKLISTED: &str = "ffa4e6181777692565cf28528fc88fd1516ea86b56da075235fa575af6a4b855";
/// A token amount unit large enough that exact votes are not dust.
const U: u64 = 1_000_000;

fn w(v: u64) -> Vec<u8> {
    let mut out = vec![0; 32];
    out[24..].copy_from_slice(&v.to_be_bytes());
    out
}
fn pad(a: &[u8; 20]) -> Vec<u8> {
    [vec![0; 12], a.to_vec()].concat()
}
fn preimage(holder: &[u8; 20], slot: u8) -> (String, Vec<u8>) {
    let pre = [pad(holder), w(u64::from(slot))].concat();
    (hex::encode(hash(&pre)), pre)
}
fn call(index: u32, parent: u32, depth: u32, kind: eth::CallType, address: [u8; 20]) -> eth::Call {
    eth::Call {
        index,
        parent_index: parent,
        depth,
        call_type: kind.into(),
        address: address.to_vec(),
        begin_ordinal: 1,
        end_ordinal: 1000,
        ..Default::default()
    }
}
fn root() -> eth::Call {
    call(1, 0, 0, eth::CallType::Call, TOKEN)
}
/// Persists the word at `key` with its same-frame `preimage`.
fn store_key(c: &mut eth::Call, key: [u8; 32], preimage: &[u8], old: Vec<u8>, new: Vec<u8>, ordinal: u64) {
    c.keccak_preimages.insert(hex::encode(key), hex::encode(preimage));
    c.storage_changes.push(eth::StorageChange {
        address: TOKEN.to_vec(),
        key: key.to_vec(),
        old_value: old,
        new_value: new,
        ordinal,
    });
}
/// Persists `holder`'s raw word under mapping `slot` with its preimage.
fn store_word(c: &mut eth::Call, holder: [u8; 20], slot: u8, old: Vec<u8>, new: Vec<u8>, ordinal: u64) {
    let (key, pre) = preimage(&holder, slot);
    store_key(c, hex::decode(key).unwrap().try_into().unwrap(), &pre, old, new, ordinal);
}
/// Persists `holder`'s word under mapping `slot` with its same-frame preimage.
fn store(c: &mut eth::Call, holder: [u8; 20], slot: u8, old: u64, new: u64, ordinal: u64) {
    store_word(c, holder, slot, w(old), w(new), ordinal);
}
fn log(topic0: &str, holders: &[[u8; 20]], value: u64) -> eth::Log {
    eth::Log {
        address: TOKEN.to_vec(),
        topics: [vec![hex::decode(topic0).unwrap()], holders.iter().map(pad).collect()].concat(),
        data: w(value),
        ..Default::default()
    }
}
fn transfer(from: [u8; 20], to: [u8; 20], value: u64) -> eth::Log {
    log(TRANSFER, &[from, to], value)
}
/// A STATICCALL `balanceOf(holder)` of the token that reads mapping `slot`.
fn balance_of(index: u32, holder: [u8; 20], slot: u8, value: u64, ordinal: u64) -> eth::Call {
    let mut c = call(index, 1, 1, eth::CallType::Static, TOKEN);
    c.input = [vec![0x70, 0xa0, 0x82, 0x31], pad(&holder)].concat();
    c.return_data = w(value);
    let (key, pre) = preimage(&holder, slot);
    c.keccak_preimages.insert(key, hex::encode(pre));
    (c.begin_ordinal, c.end_ordinal) = (ordinal, ordinal + 1);
    c
}
/// A router transaction whose calls follow the root call at depth one.
fn router(calls: Vec<eth::Call>) -> Vec<eth::Call> {
    [vec![call(1, 0, 0, eth::CallType::Call, [0x99; 20])], calls].concat()
}
fn block(txs: Vec<Vec<eth::Call>>) -> eth::Block {
    eth::Block {
        ver: 5,
        number: 7,
        hash: vec![1; 32],
        detail_level: eth::block::DetailLevel::DetaillevelExtended.into(),
        header: buffa::MessageField::some(eth::BlockHeader {
            number: 7,
            parent_hash: vec![2; 32],
            state_root: vec![3; 32],
            ..Default::default()
        }),
        transaction_traces: txs
            .into_iter()
            .enumerate()
            .map(|(i, calls)| eth::TransactionTrace {
                index: i as u32,
                from: FROM.to_vec(),
                status: eth::TransactionTraceStatus::Succeeded.into(),
                calls,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}
/// The token's inferred rows as (holder byte, amount).
fn rows(b: &eth::Block) -> Vec<(u8, String)> {
    run(b)
        .unwrap()
        .balances
        .into_iter()
        .filter(|r| r.contract.as_deref() == Some(&TOKEN[..]))
        .map(|r| (r.address[0], r.amount))
        .collect()
}
fn r(holder: [u8; 20], amount: u64) -> (u8, String) {
    (holder[0], amount.to_string())
}
/// A transfer of 30 from A (100) to B (5) under the voted mapping.
fn simple() -> eth::Call {
    let mut c = root();
    c.logs = vec![transfer(A, B, 30)];
    store(&mut c, A, BASE, 100, 70, 10);
    store(&mut c, B, BASE, 5, 35, 11);
    c
}

#[test]
fn exact_votes_emit_the_last_persisted_word_of_candidate_holders() {
    let mut first = simple();
    // C has a mapping write but no event: not a reference candidate.
    store(&mut first, C, BASE, 1, 2, 12);
    let mut second = root();
    second.logs = vec![transfer(B, A, 5)];
    store(&mut second, B, BASE, 35, 30, 20);
    store(&mut second, A, BASE, 70, 75, 21);
    let b = block(vec![vec![first], vec![second]]);
    // FROM and the token itself are candidates without a write or a read.
    assert_eq!(rows(&b), vec![r(A, 75), r(B, 30)]);
}

#[test]
fn tax_transfers_vote_by_net_flow() {
    let mut c = root();
    c.logs = vec![transfer(A, B, 90), transfer(A, FEE, 10)];
    store(&mut c, A, BASE, 100, 0, 10);
    store(&mut c, B, BASE, 0, 90, 11);
    store(&mut c, FEE, BASE, 0, 10, 12);
    assert_eq!(rows(&block(vec![vec![c]])), vec![r(A, 0), r(B, 90), r(FEE, 10)]);
    // WBTC/FiatToken: Mint repeats the zero-address Transfer, which alone votes.
    let mut wbtc = root();
    wbtc.logs = vec![log(MINT, &[B], 7), transfer([0; 20], B, 7)];
    store(&mut wbtc, B, BASE, 0, 7, 10);
    assert_eq!(rows(&block(vec![vec![wbtc]])), vec![r(B, 7)]);
    // DSToken: a Mint-only holder is written under the base that transfers chose.
    let mut ds = root();
    ds.logs = vec![log(MINT, &[C], 4)];
    store(&mut ds, C, BASE, 0, 4, 1);
    assert_eq!(rows(&block(vec![vec![ds.clone()], vec![simple()]])), vec![r(A, 70), r(B, 35), r(C, 4)]);
    // Without a Transfer from the contract in the block, nothing is inferred.
    assert!(rows(&block(vec![vec![ds]])).is_empty());
}

/// A staking pool's `Deposit(user, amount)` with `userInfo[user].amount` at
/// struct offset 0 matches the WETH shape but is not a token.
#[test]
fn deposits_alone_do_not_make_a_token() {
    let mut pool = root();
    pool.logs = vec![log(DEPOSIT, &[A], 500)];
    store(&mut pool, A, 4, 1000, 1500, 10);
    assert!(rows(&block(vec![vec![pool]])).is_empty());
}

#[test]
fn tied_or_mismatched_bases_emit_nothing() {
    let mut mirror = simple();
    store(&mut mirror, A, 4, 100, 70, 12);
    store(&mut mirror, B, 4, 5, 35, 13);
    assert!(rows(&block(vec![vec![mirror]])).is_empty());
    let mut fee = root();
    fee.logs = vec![transfer(A, B, 30)];
    store(&mut fee, A, BASE, 100, 71, 10);
    store(&mut fee, B, BASE, 5, 34, 11);
    assert!(rows(&block(vec![vec![fee]])).is_empty());
    // One exact side outvotes one mismatched side, unless it is dust.
    let silent_fee = |u: u64| {
        let mut c = root();
        c.logs = vec![transfer(A, B, 30 * u)];
        store(&mut c, A, BASE, 100 * u, 69 * u, 10);
        store(&mut c, B, BASE, 5 * u, 35 * u, 11);
        store(&mut c, TOKEN, BASE, 0, u, 12);
        block(vec![vec![c]])
    };
    assert_eq!(rows(&silent_fee(U)), vec![r(TOKEN, U), r(A, 69 * U), r(B, 35 * U)]);
    assert!(rows(&silent_fee(1)).is_empty());
}

/// An allowance `allowance[A][A]` that moves with A's `transferFrom` is an
/// inner mapping, never a balance.
#[test]
fn inner_mappings_are_never_chosen() {
    let mut c = root();
    c.logs = vec![transfer(A, B, 30)];
    let (inner, inner_pre) = preimage(&A, 1);
    let inner: [u8; 32] = hex::decode(inner).unwrap().try_into().unwrap();
    store_key(&mut c, inner, &inner_pre, w(0), w(0), 9);
    let outer_pre = [pad(&A), inner.to_vec()].concat();
    store_key(&mut c, hash(&outer_pre), &outer_pre, w(1000), w(970), 10);
    assert!(rows(&block(vec![vec![c]])).is_empty());
}

#[test]
fn reverted_frames_neither_vote_nor_persist() {
    let mut parent = simple();
    parent.end_ordinal = 100;
    let mut reverted = call(2, 1, 1, eth::CallType::Call, TOKEN);
    reverted.state_reverted = true;
    reverted.logs = vec![transfer(B, A, 1)];
    store(&mut reverted, B, BASE, 35, 34, 30);
    store(&mut reverted, A, BASE, 70, 71, 31);
    assert_eq!(rows(&block(vec![vec![parent, reverted.clone()]])), vec![r(A, 70), r(B, 35)]);
    let mut only = simple();
    only.state_reverted = true;
    assert!(rows(&block(vec![vec![only]])).is_empty());
}

#[test]
fn delegate_frames_match_proxy_logs_and_writes() {
    let proxy = root();
    let mut implementation = simple();
    (implementation.index, implementation.parent_index, implementation.depth) = (2, 1, 1);
    implementation.call_type = eth::CallType::Delegate.into();
    implementation.address = IMPL.to_vec();
    assert_eq!(rows(&block(vec![vec![proxy, implementation]])), vec![r(A, 70), r(B, 35)]);
}

#[test]
fn contradicting_or_impure_balance_of_reads_exclude_the_token() {
    let with = |reads: Vec<eth::Call>| rows(&block(vec![vec![simple()], router(reads)]));
    assert_eq!(with(vec![balance_of(2, B, BASE, 35, 50)]), vec![r(A, 70), r(B, 35)]);
    assert!(with(vec![balance_of(2, B, BASE, 36, 50)]).is_empty());
    // A read before the write is checked against the write's old word.
    assert!(rows(&block(vec![router(vec![balance_of(2, B, BASE, 6, 5)]), vec![simple()]])).is_empty());
    // balanceOf also reads another holder mapping, or reads a different one.
    let mut other = balance_of(2, B, BASE, 35, 50);
    let (key, pre) = preimage(&B, 9);
    other.keccak_preimages.insert(key, hex::encode(pre));
    assert!(with(vec![other]).is_empty());
    assert!(with(vec![balance_of(2, B, 9, 35, 50)]).is_empty());
    // A call out of the token's storage context, unless it is a beacon lookup.
    let mut outside = call(3, 2, 2, eth::CallType::Static, [0x55; 20]);
    outside.return_data = w(1);
    assert!(with(vec![balance_of(2, B, BASE, 35, 50), outside]).is_empty());
    let mut proxy = balance_of(2, B, BASE, 35, 50);
    proxy.keccak_preimages.clear();
    let mut lookup = call(3, 2, 2, eth::CallType::Static, BEACON);
    lookup.return_data = pad(&IMPL);
    let mut delegate = balance_of(4, B, BASE, 35, 50);
    (delegate.parent_index, delegate.depth, delegate.call_type, delegate.address) = (2, 2, eth::CallType::Delegate.into(), IMPL.to_vec());
    assert_eq!(with(vec![proxy, lookup, delegate]), vec![r(A, 70), r(B, 35)]);
}

#[test]
fn pure_reads_supply_unwritten_candidate_holders() {
    let with = |reads: Vec<eth::Call>| rows(&block(vec![vec![simple()], router(reads)]));
    assert_eq!(with(vec![balance_of(2, FROM, BASE, 42, 50)]), vec![r(A, 70), r(B, 35), r(FROM, 42)]);
    // C is not a candidate; two different unchanged reads are a contradiction.
    assert_eq!(with(vec![balance_of(2, C, BASE, 42, 50)]), vec![r(A, 70), r(B, 35)]);
    assert!(with(vec![balance_of(2, FROM, BASE, 42, 50), balance_of(3, FROM, BASE, 43, 60)]).is_empty());
}

/// Producers on firehose-tracer 5.5.0 and later drop the preimages that explain
/// no storage write of their transaction, so a `balanceOf` of an unchanged
/// holder usually records none. Such an unkeyed read still excludes the token
/// when it contradicts the stored word or calls out, but not for lacking a
/// preimage, and it never chooses the base, supplies an unwritten holder or
/// vouches for an unexplained write.
#[test]
fn unkeyed_reads_only_veto() {
    let unkeyed = |index: u32, holder: [u8; 20], value: u64| {
        let mut c = balance_of(index, holder, BASE, value, 50);
        c.keccak_preimages.clear();
        c
    };
    let with = |txs: Vec<Vec<eth::Call>>, reads: Vec<eth::Call>| rows(&block([txs, vec![router(reads)]].concat()));
    assert_eq!(with(vec![vec![simple()]], vec![unkeyed(2, B, 35)]), vec![r(A, 70), r(B, 35)]);
    assert!(with(vec![vec![simple()]], vec![unkeyed(2, B, 36)]).is_empty());
    let mut outside = call(3, 2, 2, eth::CallType::Static, [0x55; 20]);
    outside.return_data = w(1);
    assert!(with(vec![vec![simple()]], vec![unkeyed(2, B, 35), outside]).is_empty());
    assert_eq!(with(vec![vec![simple()]], vec![unkeyed(2, FROM, 42)]), vec![r(A, 70), r(B, 35)]);
    let mut mirror = simple();
    store(&mut mirror, A, 4, 100, 70, 12);
    store(&mut mirror, B, 4, 5, 35, 13);
    assert!(with(vec![vec![mirror]], vec![unkeyed(2, B, 35)]).is_empty());
    let mut sync = root();
    sync.logs = vec![log(APPROVAL, &[C, D], 1)];
    store(&mut sync, C, BASE, 50, 60, 20);
    assert_eq!(with(vec![vec![simple()], vec![sync]], vec![unkeyed(2, C, 60)]), vec![r(A, 70), r(B, 35)]);
}

/// Producers on firehose-tracer 5.5.0 and later cut a transaction's later
/// internal calls to their selector past 50 MiB of call input, and drop their
/// return data past 25 MiB. A successful `balanceOf` of the token without a
/// 36-byte input or return data may be a read whose veto was lost, so it drops
/// the token's rows.
#[test]
fn truncated_balance_of_calls_drop_the_token() {
    let with = |read: eth::Call| rows(&block(vec![vec![simple()], router(vec![read])]));
    let mut input = balance_of(2, B, BASE, 35, 50);
    input.input.truncate(4);
    assert!(with(input.clone()).is_empty());
    let mut output = balance_of(2, B, BASE, 35, 50);
    output.return_data.clear();
    assert!(with(output).is_empty());
    // A failed call, or a call to another contract, is not a read.
    input.status_failed = true;
    assert_eq!(with(input.clone()), vec![r(A, 70), r(B, 35)]);
    (input.status_failed, input.address) = (false, [0x55; 20].to_vec());
    assert_eq!(with(input), vec![r(A, 70), r(B, 35)]);
}

/// A reflection-style second mapping (`rOwned`) that moves with the flows
/// suggests a computed `balanceOf`. A traced read vouches only for its holder,
/// whose code path may differ (e.g. `holder == pair ? tOwned : rOwned / rate`).
#[test]
fn other_holder_state_moving_with_transfers_needs_the_holders_own_read() {
    let mut c = root();
    c.logs = vec![transfer(C, B, 30), transfer(A, C, 20)];
    store(&mut c, C, BASE, 1000, 990, 10);
    store(&mut c, B, BASE, 5, 35, 11);
    store(&mut c, A, BASE, 100, 80, 12);
    store(&mut c, C, 4, 1_000_000, 990_000, 13);
    store(&mut c, B, 4, 6_000, 36_000, 14);
    store(&mut c, A, 4, 101_000, 81_000, 15);
    assert!(rows(&block(vec![vec![c.clone()]])).is_empty());
    let read = router(vec![balance_of(2, C, BASE, 990, 50)]);
    assert_eq!(rows(&block(vec![vec![c], read])), vec![r(C, 990)]);
}

/// Counters (deltas below a thousandth of the flow) do not shadow a base with
/// two or more exact votes, unless another mapping also matches exactly or the
/// base has a single exact vote.
#[test]
fn counters_do_not_shadow_a_well_supported_base() {
    let counted = |extra: &dyn Fn(&mut eth::Call)| {
        let mut c = root();
        c.logs = vec![transfer(A, B, 30 * U)];
        store(&mut c, A, BASE, 100 * U, 70 * U, 10);
        store(&mut c, B, BASE, 5 * U, 35 * U, 11);
        store(&mut c, A, 7, 1, 2, 12);
        store(&mut c, B, 7, 1, 2, 13);
        extra(&mut c);
        rows(&block(vec![vec![c]]))
    };
    assert_eq!(counted(&|_| {}), vec![r(A, 70 * U), r(B, 35 * U)]);
    assert!(counted(&|c| store(c, A, 5, 100 * U, 70 * U, 14)).is_empty());
    let mut mint = root();
    mint.logs = vec![transfer([0; 20], B, 30 * U)];
    store(&mut mint, B, BASE, 5 * U, 35 * U, 10);
    store(&mut mint, B, 7, 1, 2, 11);
    assert!(rows(&block(vec![vec![mint]])).is_empty());
}

/// A tie between two exactly moving mappings is broken by the traced reads.
#[test]
fn pure_reads_choose_the_base_on_a_tie() {
    let mut mirror = simple();
    store(&mut mirror, A, 4, 100, 70, 12);
    store(&mut mirror, B, 4, 5, 35, 13);
    let reads = router(vec![balance_of(2, B, BASE, 35, 50)]);
    assert_eq!(rows(&block(vec![vec![mirror], reads])), vec![r(A, 70), r(B, 35)]);
}

/// Scaled balances (stored = amount / index, Aave-style): a dust transfer that
/// rounds to an exact match does not certify the mapping.
#[test]
fn dust_matches_do_not_certify_a_mismatching_base() {
    let mut big = root();
    big.logs = vec![transfer(A, B, 1000 * U)];
    store(&mut big, A, BASE, 2000 * U, 1048 * U, 10);
    store(&mut big, B, BASE, 0, 952 * U, 11);
    let mut dust = root();
    dust.logs = vec![transfer(C, FEE, 1)];
    store(&mut dust, C, BASE, 500, 499, 10);
    store(&mut dust, FEE, BASE, 0, 1, 11);
    assert!(rows(&block(vec![vec![big], vec![dust]])).is_empty());
}

#[test]
fn self_destructed_contracts_are_not_inferred() {
    let mut destroyed = simple();
    destroyed.suicide = true;
    assert!(rows(&block(vec![vec![destroyed]])).is_empty());
    let mut removed = simple();
    removed.code_changes = vec![eth::CodeChange {
        address: TOKEN.to_vec(),
        old_hash: vec![1; 32],
        new_hash: hash(&[]).to_vec(),
        ordinal: 20,
        ..Default::default()
    }];
    assert!(rows(&block(vec![vec![removed]])).is_empty());
}

/// Many interleaved writes and reads of one word are checked in ordinal
/// order (binary search, not a scan per read).
#[test]
fn interleaved_reads_check_the_word_at_their_ordinal() {
    let n = 2000;
    let mut c = root();
    c.logs = vec![transfer(A, B, 30 * U)];
    store(&mut c, B, BASE, 5 * U, 35 * U, 1);
    let mut value = 100 * U;
    let mut reads = Vec::new();
    for i in 0..n {
        let next = if i + 1 == n { 70 * U } else { value - 1 };
        store(&mut c, A, BASE, value, next, 10 + 10 * i);
        reads.push(balance_of(2 + i as u32, A, BASE, next, 15 + 10 * i));
        value = next;
    }
    let b = |reads: Vec<eth::Call>| block(vec![vec![c.clone()], router(reads)]);
    assert_eq!(rows(&b(reads.clone())), vec![r(A, 70 * U), r(B, 35 * U)]);
    reads[n as usize / 2].return_data = w(1);
    assert!(rows(&b(reads)).is_empty());
}

/// `Deposit`/`Withdrawal`/`Mint`/`Burn` name holders but never vote: a staking
/// pool's `userInfo[user].amount` or a lock ledger moving with them is not a
/// balance.
#[test]
fn credits_and_debits_never_vote() {
    let staking = |topic0: &str, holder: [u8; 20], before: u64, after: u64| {
        let mut c = root();
        c.logs = vec![log(topic0, &[holder], before.abs_diff(after))];
        store(&mut c, holder, 4, before, after, 10);
        c
    };
    // A first stake that pays a zero reward with Transfer(TOKEN, A, 0).
    let mut first = staking(DEPOSIT, A, 0, 500 * U);
    first.logs.push(transfer(TOKEN, A, 0));
    assert!(rows(&block(vec![vec![first]])).is_empty());
    // Stakes and an unstake do not outvote one ordinary transfer.
    let mut t = root();
    t.logs = vec![transfer(D, E, 10 * U)];
    store(&mut t, D, BASE, 100 * U, 90 * U, 10);
    store(&mut t, E, BASE, 5 * U, 15 * U, 11);
    let txs = vec![
        vec![staking(DEPOSIT, A, 0, 500 * U)],
        vec![staking(DEPOSIT, B, 40 * U, 100 * U)],
        vec![staking(WITHDRAWAL, C, 9 * U, 3 * U)],
        vec![t],
    ];
    assert_eq!(rows(&block(txs)), vec![r(D, 90 * U), r(E, 15 * U)]);
    // A stake that also pays a reward: only the Transfer is A's balance flow.
    let mut reward = staking(DEPOSIT, A, 0, 500 * U);
    reward.logs.push(transfer(TOKEN, A, 2 * U));
    store(&mut reward, TOKEN, BASE, 100 * U, 98 * U, 11);
    store(&mut reward, A, BASE, 0, 2 * U, 12);
    assert_eq!(rows(&block(vec![vec![reward]])), vec![r(TOKEN, 98 * U), r(A, 2 * U)]);
    // A batch lock-mint: Mint per beneficiary, one Transfer to the contract.
    let mut lock = root();
    lock.logs = vec![log(MINT, &[A], 300 * U), log(MINT, &[B], 200 * U), transfer([0; 20], TOKEN, 500 * U)];
    store(&mut lock, A, 6, 0, 300 * U, 10);
    store(&mut lock, B, 6, 0, 200 * U, 11);
    store(&mut lock, TOKEN, BASE, 1000 * U, 1500 * U, 12);
    assert_eq!(rows(&block(vec![vec![lock]])), vec![r(TOKEN, 1500 * U)]);
}

/// A packed word `lastBlock << 128 | balance`: a holder's first touch in the
/// block also moves the block field (a large mismatch), later touches only the
/// balance (exact). More large mismatches than exact votes reject the mapping.
#[test]
fn packed_words_whose_other_field_moves_are_not_chosen() {
    let packed = |high: u64, low: u64| {
        let mut out = w(low);
        out[8..16].copy_from_slice(&high.to_be_bytes());
        out
    };
    let (mut pair, mut last) = (1000 * U, 480);
    let mut txs = Vec::new();
    for (i, buyer) in [A, B, C].into_iter().enumerate() {
        let (amount, at) = ((10 + i as u64) * U, 10 * i as u64);
        let mut c = root();
        c.logs = vec![transfer(FEE, buyer, amount)];
        store_word(&mut c, FEE, BASE, packed(last, pair), packed(500, pair - amount), 10 + at);
        store_word(&mut c, buyer, BASE, packed(477, U), packed(500, U + amount), 11 + at);
        (pair, last) = (pair - amount, 500);
        txs.push(vec![c]);
    }
    assert!(rows(&block(txs)).is_empty());
}

/// A third party's word that changes with no balance event naming it in that
/// frame (FiatToken v2.2 sets a blacklist flag in bit 255) needs the holder's
/// own `balanceOf`; the contract's own silent fee writes do not.
#[test]
fn unexplained_writes_need_the_holders_own_read() {
    let mut flag = root();
    flag.logs = vec![eth::Log {
        address: TOKEN.to_vec(),
        topics: vec![hex::decode(BLACKLISTED).unwrap(), pad(&C)],
        ..Default::default()
    }];
    let mut flagged = w(50 * U);
    flagged[0] |= 0x80;
    store_word(&mut flag, C, BASE, w(50 * U), flagged, 20);
    assert_eq!(rows(&block(vec![vec![simple()], vec![flag]])), vec![r(A, 70), r(B, 35)]);
    // An approval makes C a candidate; its silent sync is kept once C's own
    // balanceOf confirms the word.
    let mut sync = root();
    sync.logs = vec![log(APPROVAL, &[C, D], 1)];
    store(&mut sync, C, BASE, 50, 60, 20);
    assert_eq!(rows(&block(vec![vec![simple()], vec![sync.clone()]])), vec![r(A, 70), r(B, 35)]);
    let read = router(vec![balance_of(2, C, BASE, 60, 50)]);
    assert_eq!(rows(&block(vec![vec![simple()], vec![sync], read])), vec![r(A, 70), r(B, 35), r(C, 60)]);
}

/// Recorded preimages longer than 64 bytes are never decoded, however often
/// their key is written.
#[test]
fn long_preimages_are_ignored() {
    let mut c = simple();
    let long = vec![7; 4096];
    for i in 0..2000 {
        store_key(&mut c, hash(&long), &long, w(i), w(i + 1), 100 + i);
    }
    assert_eq!(rows(&block(vec![vec![c]])), vec![r(A, 70), r(B, 35)]);
}

/// Only an incomplete Extended block or unresolvable persistence fails the
/// block. Doubt about a contract, such as a malformed storage word, drops its
/// rows for the block instead.
#[test]
fn blocks_fail_only_on_incomplete_extended_data() {
    let fails = |b: &eth::Block| run(b).unwrap_err().to_string();
    let mut b = block(vec![vec![simple()]]);
    b.detail_level = eth::block::DetailLevel::DetaillevelBase.into();
    assert_eq!(fails(&b), "Extended blocks required");
    b = block(vec![vec![simple()]]);
    b.ver = 2;
    assert_eq!(fails(&b), "unsupported Extended producer version");
    assert_eq!(fails(&block(vec![vec![simple()], vec![]])), "incomplete transaction persistence data");
    // A failed SetCode transaction without an authorization/execution boundary.
    b = block(vec![vec![root()]]);
    let tx = &mut b.transaction_traces[0];
    tx.status = eth::TransactionTraceStatus::Failed.into();
    tx.r#type = eth::transaction_trace::Type::TrxTypeSetCode.into();
    tx.set_code_authorizations.push(eth::SetCodeAuthorization {
        authority: Some(C.to_vec()),
        ..Default::default()
    });
    tx.calls[0].begin_ordinal = 0;
    assert!(fails(&b).contains("no authorization/execution ordinal boundary"));
    let mut c = simple();
    c.storage_changes[0].new_value = vec![1; 33];
    assert!(run(&block(vec![vec![c]])).unwrap().balances.is_empty());
}

/// On captured BSC block 122260950, inferred rows equal the saved same-block
/// RPC values wherever those exist: all 18 recorded ERC-20 `balanceOf` checks,
/// and the reference rows of USDT, WBNB, USDC and BTCB (101 of 136 inferred).
#[test]
fn captured_block_matches_the_saved_rpc_balances() {
    use std::collections::{BTreeMap, BTreeSet};
    let block = eth::Block::decode_from_slice(include_bytes!("fixtures/bsc-122260950.pb").as_slice()).unwrap();
    let ours: BTreeMap<_, _> = run(&block)
        .unwrap()
        .balances
        .into_iter()
        .map(|b| {
            (
                (format!("0x{}", hex::encode(b.contract.unwrap())), format!("0x{}", hex::encode(b.address))),
                b.amount,
            )
        })
        .collect();
    assert_eq!(ours.len(), 164);
    let key = |row: &serde_json::Value| {
        (
            row["contract"].as_str().unwrap().to_lowercase(),
            row["address"].as_str().unwrap().to_lowercase(),
        )
    };
    let checks: serde_json::Value = serde_json::from_str(include_str!("fixtures/bsc-122260950.json")).unwrap();
    let erc20: Vec<_> = checks["rpc_checks"].as_array().unwrap().iter().filter(|c| c["contract"] != "").collect();
    assert_eq!(erc20.len(), 18);
    for check in erc20 {
        assert_eq!(ours.get(&key(check)).map(String::as_str), check["balance"].as_str(), "{check}");
    }
    let core: serde_json::Value = serde_json::from_str(include_str!("fixtures/bsc-122260950-core-reference.json")).unwrap();
    let reference: BTreeMap<_, _> = core["balances"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (key(r), r["amount"].as_str().unwrap()))
        .collect();
    let tokens: BTreeSet<_> = reference.keys().map(|k| &k.0).collect();
    let inferred: Vec<_> = ours.iter().filter(|(k, _)| tokens.contains(&k.0)).collect();
    assert_eq!((reference.len(), tokens.len(), inferred.len()), (136, 4, 101));
    assert!(inferred.iter().all(|(k, v)| reference.get(*k) == Some(&v.as_str())));
}

/// Captured block 122260950 as a firehose-tracer 5.5.0 producer would record
/// it, without the 383 preimages that explain no storage write of their
/// transaction (`fixtures/bsc-122260950-5.5.0-dropped-preimages.json`). 13 of
/// its `balanceOf` calls lose their holder's preimage and every row stays the
/// same; while unkeyed reads vetoed, 5 tokens lost all 104 of their rows here.
#[test]
fn captured_block_without_read_preimages_keeps_its_rows() {
    let original = eth::Block::decode_from_slice(include_bytes!("fixtures/bsc-122260950.pb").as_slice()).unwrap();
    let dropped: serde_json::Value = serde_json::from_str(include_str!("fixtures/bsc-122260950-5.5.0-dropped-preimages.json")).unwrap();
    let mut filtered = original.clone();
    for d in dropped["dropped"].as_array().unwrap() {
        let (tx, index, hash) = (d[0].as_u64().unwrap() as usize, d[1].as_u64().unwrap() as u32, d[2].as_str().unwrap());
        let call = filtered.transaction_traces[tx].calls.iter_mut().find(|c| c.index == index).unwrap();
        assert!(call.keccak_preimages.remove(hash).is_some(), "{d}");
    }
    // `balanceOf` calls that recorded `pad(holder) || base` in their own frame.
    let keyed = |b: &eth::Block| {
        let holder = |c: &eth::Call| format!("{:0>64}", hex::encode(&c.input[16..]));
        b.transaction_traces
            .iter()
            .flat_map(|tx| &tx.calls)
            .filter(|c| c.input.len() == 36 && c.input[..4] == [0x70, 0xa0, 0x82, 0x31])
            .filter(|c| c.keccak_preimages.values().any(|p| p.len() == 128 && p[..64] == holder(c)))
            .count()
    };
    assert_eq!((keyed(&original), keyed(&filtered)), (175, 162));
    let all = |b: &eth::Block| {
        run(b)
            .unwrap()
            .balances
            .into_iter()
            .map(|r| (r.contract, r.address, r.amount))
            .collect::<Vec<_>>()
    };
    assert_eq!(all(&filtered).len(), 164);
    assert_eq!(all(&filtered), all(&original));
}

#[test]
fn package_has_one_block_only_map_and_only_the_shared_protobuf() {
    let manifest = include_str!("../substreams.yaml");
    let modules = manifest.lines().filter(|line| line.starts_with("  - name:")).collect::<Vec<_>>();
    assert_eq!(modules, vec!["  - name: map_events"]);
    assert!(manifest.contains("    inputs:\n      - source: sf.ethereum.type.v2.Block\n    output:\n"));
    assert!(!manifest.contains("params"));
    assert!(manifest.contains("files: [balances.proto]"));
    assert!(manifest.contains("type: proto:evm.balances.v1.Events"));
}
