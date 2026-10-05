//! RPC-free transfer-guided balances, inferred for every contract from the
//! block's persisted storage. Per block and per contract:
//!
//! 1. Candidates are the RPC reference's `(contract, holder)` pairs: see
//!    [`candidates`].
//! 2. Every non-reverted call frame of a successful transaction votes. A holder's
//!    signed net `Transfer` flow in the frame is compared with the net delta of
//!    the frame's persisted writes whose key has a verified same-frame Keccak
//!    preimage `pad(holder) || base`. Equal is an exact vote for `base`, unequal
//!    a mismatch. Exact votes for flows below 10^6 do not count for a base that
//!    also mismatches. `Deposit`/`Withdrawal`/`Mint`/`Burn` never vote.
//! 3. The base is the mapping every traced `balanceOf` reads, if it has exact
//!    votes, or else the base with the most exact votes; a tie chooses none.
//!    Inner (nested) mappings are never chosen, nor a base whose own large
//!    mismatches outnumber its exact votes.
//! 4. The contract is excluded for the block when a persisted record or event
//!    address is malformed, its code is removed or it self-destructs, writes to
//!    one key are discontinuous, or a traced `balanceOf` reads other holder
//!    state, calls out, or returns a value that contradicts storage.
//! 5. When other holder mappings mismatch the event flows at least as often as
//!    the chosen one matches them, only holders with their own traced
//!    `balanceOf` keep rows. With two or more exact votes and no other mapping
//!    matching, only mismatches of at least a thousandth of the flow count.
//! 6. Candidate holders written under the base get their last persisted word,
//!    unless that write is unexplained: a holder other than the contract whose
//!    last write's frame has no `Transfer`/`Deposit`/`Withdrawal`/`Mint`/`Burn`
//!    naming it needs its own traced `balanceOf`. Candidate holders without a
//!    write get the value of a traced `balanceOf` that read exactly the base.
//!
//! Inference never fails a block: any detected doubt drops that contract's
//! inferred rows for the block. Holders without a write or such a read stay
//! unknown. Per-block work is bounded by the block's calls, logs, writes and
//! 64-byte preimages; longer recorded preimages are never decoded.
use crate::{eth, hash, mapping, persist};
use std::collections::{BTreeMap, BTreeSet};

type Addr = [u8; 20];
type Word = [u8; 32];
/// (system call, transaction index, call index)
type Frame = (bool, u32, u32);
/// Net `Transfer` flow per holder of one contract in one frame.
type Flows<'a> = BTreeMap<(Frame, Addr), (&'a eth::Call, BTreeMap<Addr, Net>)>;
/// Per contract, the `(frame, holder)` pairs that a balance event names.
type Named = BTreeMap<Addr, BTreeSet<(Frame, Addr)>>;

const SUCCEEDED: i32 = eth::TransactionTraceStatus::Succeeded as i32;
const CALL: i32 = eth::CallType::Call as i32;
const STATIC: i32 = eth::CallType::Static as i32;
const DELEGATE: i32 = eth::CallType::Delegate as i32;
const BALANCE_OF: [u8; 4] = [0x70, 0xa0, 0x82, 0x31];
const ZERO: Addr = [0; 20];
const TRANSFER: Word = topic("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");
/// WETH `Deposit`/`Withdrawal` and the `Mint`/`Burn(address indexed, uint256)`
/// of DSToken, WBTC, FiatToken and Tether: they name a holder whose balance
/// changes, but may also mean staking or locking, so they never vote.
const CREDITS_DEBITS: [Word; 4] = [
    topic("e1fffcc4923d04b559f4d29a8bfc6cda04eb5b0d3c460751c2402c5c5cc9109c"),
    topic("7fcf532c15f0a6db0bd6d0e038bea71d30d808c7d98cb3bf7268a95bf5081b65"),
    topic("0f6798a560793a54c3bcfe86a93cde1e73087d944c0ea20544137d4121396885"),
    topic("cc16f5dbb4873280815c1ee09dbd06736cffcc184412cf7a71a0fdb75d397ca5"),
];

const fn nibble(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        _ => panic!("lowercase hex expected"),
    }
}
const fn topic(hex: &str) -> Word {
    let b = hex.as_bytes();
    assert!(b.len() == 64);
    let mut out = [0; 32];
    let mut i = 0;
    while i < 32 {
        out[i] = (nibble(b[2 * i]) << 4) | nibble(b[2 * i + 1]);
        i += 1;
    }
    out
}

/// Which addresses a matched reference event contributes as holders.
#[derive(Clone, Copy)]
enum Part {
    /// Only `tx.from` and the emitting contract.
    Context,
    Topic1,
    Topic2,
    Topics12,
    Word0,
    Words01,
    /// The caller of the frame that emitted the log.
    Caller,
    /// The reference's holder is an `owner()` RPC result: not reproducible.
    RpcOwner,
}
struct Shape {
    topic0: Word,
    topics: usize,
    data: usize,
    part: Part,
}
const fn shape(topic0: &str, topics: usize, data: usize, part: Part) -> Shape {
    Shape {
        topic0: topic(topic0),
        topics,
        data,
        part,
    }
}
/// The event shapes of the RPC reference (substreams-evm erc20-balances v0.3.4
/// with its erc20-transfers and erc20-tokens inputs): topic0, exact topic count
/// and exact data length, with no address filter.
const SHAPES: [Shape; 64] = [
    shape("e1fffcc4923d04b559f4d29a8bfc6cda04eb5b0d3c460751c2402c5c5cc9109c", 2, 32, Part::Topic1), // weth.Deposit
    shape("7fcf532c15f0a6db0bd6d0e038bea71d30d808c7d98cb3bf7268a95bf5081b65", 2, 32, Part::Topic1), // weth.Withdrawal
    shape("ab8530f87dc9b59234c4623bf917212bb2536d647574c8e7e5da92c2ede0c9f8", 3, 32, Part::Topics12), // fiattokenv2_2.Mint
    shape("cc16f5dbb4873280815c1ee09dbd06736cffcc184412cf7a71a0fdb75d397ca5", 2, 32, Part::Topic1), // fiattokenv2_2.Burn
    shape("ffa4e6181777692565cf28528fc88fd1516ea86b56da075235fa575af6a4b855", 2, 0, Part::Topic1),  // fiattokenv2_2.Blacklisted
    shape("c67398012c111ce95ecb7429b933096c977380ee6c421175a71a4a4c6c88c06e", 2, 0, Part::Context), // fiattokenv2_2.BlacklisterChanged
    shape("db66dfa9c6b8f5226fe9aac7e51897ae8ee94ac31dc70bb6c9900b2574b707e6", 2, 0, Part::Context), // fiattokenv2_2.MasterMinterChanged
    shape("46980fca912ef9bcdbd36877427b6b90e860769f604e89c0e67720cece530d20", 2, 32, Part::Context), // fiattokenv2_2.MinterConfigured
    shape("e94479a9f7e1952cc78f2d6baab678adc1b772d936c6583def489e524cb66692", 2, 0, Part::Context), // fiattokenv2_2.MinterRemoved
    shape("8be0079c531659141344cd1fd0a4f28419497f9722a3daafe3b4186f6b6457e0", 1, 64, Part::Words01), // fiattokenv2_2.OwnershipTransferred
    shape("6985a02210a168e66602d3235cb6db0e70f92b3ba4d376a33c0f3d9434bff625", 1, 0, Part::Context), // fiattokenv2_2.Pause
    shape("b80482a293ca2e013eda8683c9bd7fc8347cfdaeea5ede58cba46df502c2a604", 2, 0, Part::Context), // fiattokenv2_2.PauserChanged
    shape("e475e580d85111348e40d8ca33cfdd74c30fe1655c2d8537a13abc10065ffa5a", 2, 0, Part::Context), // fiattokenv2_2.RescuerChanged
    shape("117e3210bb9aa7d9baff172026820255c6f6c30ba8999d1c2fd88e2848137c4e", 2, 0, Part::Topic1),  // fiattokenv2_2.UnBlacklisted
    shape("7805862f689e2f13df9f062ff482ad3ad112aca9e0847911ed832e158c525b33", 1, 0, Part::Context), // fiattokenv2_2.Unpause
    shape("1cdd46ff242716cdaa72d159d339a485b3438398348d68f09d7c8c0a59353d81", 3, 0, Part::Context), // fiattokenv2_2.AuthorizationCanceled
    shape("98de503528ee59b575ef0c0a2576a82497bfc029a5685b209e9ec333479b10a5", 3, 0, Part::Context), // fiattokenv2_2.AuthorizationUsed
    shape("cb8241adb0c3fdb35b70c24ce35c5eb0c17af7431c99f827d44a445ca624176a", 1, 32, Part::RpcOwner), // tethertoken.Issue
    shape("702d5967f45f6513a38ffc42d6ba9bf230bd40e8f53b16363c7eb4fd2deb9a44", 1, 32, Part::RpcOwner), // tethertoken.Redeem
    shape("cc358699805e9a8b7f77b522628c7cb9abd07d9efb86b6fb616af1609036a99e", 1, 32, Part::Context), // tethertoken.Deprecate
    shape("b044a1e409eac5c48e5af22d4af52670dd1a99059537a78b31b48c6500a6354e", 1, 64, Part::Context), // tethertoken.Params
    shape("61e6e66b0d6339b2980aecc6ccc0039736791f0ccde9ed512e789a7fbdd698c6", 1, 64, Part::Word0),  // tethertoken.DestroyedBlackFunds
    shape("42e160154868087d6bfdc0ca23d96a1c1cfa32f1b72ba9ba27b69b98a0d819dc", 1, 32, Part::Word0),  // tethertoken.AddedBlackList
    shape("d7e9ec6e6ecd65492dce6bf513cd6867560d49544421d0783ddf06e76c24470c", 1, 32, Part::Word0),  // tethertoken.RemovedBlackList
    shape("406bbf2d8d145125adf1198d2cf8a67c66cc4bb0ab01c37dccd4f7c0aae1e7c7", 2, 0, Part::Topic1),  // tethertoken.BlockPlaced
    shape("665918c9e02eb2fd85acca3969cb054fc84c138e60ec4af22ab6ef2fd4c93c27", 2, 0, Part::Topic1),  // tethertoken.BlockReleased
    shape("0f6798a560793a54c3bcfe86a93cde1e73087d944c0ea20544137d4121396885", 2, 32, Part::Topic1), // tethertoken.Mint
    shape("6a2859ae7902313752498feb80a014e6e7275fe964c79aa965db815db1c7f1e9", 2, 32, Part::Topic1), // tethertoken.DestroyedBlockedFunds
    shape("783cda63e44f9c07bbe4cf839a147c52931d0cd508a4930af9d1656a1201f036", 2, 0, Part::Context), // tethertoken.NewPrivilegedContract
    shape("dda50bf0570e969140ea1a415f2cd2636674ab5a19402b052918462b58c4297e", 2, 0, Part::Context), // tethertoken.RemovedPrivilegedContract
    shape("05d0634fe981be85c22e2942a880821b70095d84e152c3ea3c17a4e4250d9d61", 3, 32, Part::Topic2), // erc20swapasset.LogSwapin
    shape("6b616089d04950dc06c45c6dd787d657980543f89651aec47924752c7d16c888", 3, 32, Part::Topic1), // erc20swapasset.LogSwapout
    shape("e1968d4263a733e2597ef67ea6ad267343bba5f8bf0f99d85190e06b05d824d9", 4, 0, Part::Context), // erc20swapasset.LogChangeDcrmOwner
    shape("0f6798a560793a54c3bcfe86a93cde1e73087d944c0ea20544137d4121396885", 2, 32, Part::Topic1), // wbtc.Mint
    shape("cc16f5dbb4873280815c1ee09dbd06736cffcc184412cf7a71a0fdb75d397ca5", 2, 32, Part::Topic1), // wbtc.Burn
    shape("ae5184fba832cb2b1f702aca6117b8d265eaf03ad33eb133f19dde0f5920fa08", 1, 0, Part::Context), // wbtc.MintFinished
    shape("f8df31144d9c2f0f6b59d69b8b98abd5459d07f2742c4df920b25aae33c64820", 2, 0, Part::Context), // wbtc.OwnershipRenounced
    shape("0f6798a560793a54c3bcfe86a93cde1e73087d944c0ea20544137d4121396885", 2, 32, Part::Topic1), // sai.Mint
    shape("cc16f5dbb4873280815c1ee09dbd06736cffcc184412cf7a71a0fdb75d397ca5", 2, 32, Part::Topic1), // sai.Burn
    shape("1abebea81bfa2637f28358c371278fb15ede7ea8dd28d2e03b112ff6d936ada4", 2, 0, Part::Context), // sai.LogSetAuthority
    shape("ce241d7ca1f669fee44b6fc00b8eba2df3bb514eed0f6f668f8f89096e81ed94", 2, 0, Part::Context), // sai.LogSetOwner
    shape("96a25c8ce0baabc1fdefd93e9ed25d8e092a3332f3aa9a41722b5697231d1d1a", 2, 64, Part::Topic1), // steth.Submitted
    shape("76a397bea5768d4fca97ef47792796e35f98dc81b16c1de84e28a818e1f97108", 1, 32, Part::Context), // steth.Unbuffered
    shape("ff08c3ef606d198e316ef5b822193c489965899eb4e3c248cea1a4626c3eda50", 2, 192, Part::Context), // steth.TokenRebased
    shape("9d9c909296d9c674451c0c24f02cb64981eb3b727f99865939192f880a755dcb", 3, 32, Part::Topics12), // steth.TransferShares
    shape("8b2a1e1ad5e0578c3dd82494156e985dade827a87c573b5c1c7716a32162ad64", 2, 96, Part::Topic1), // steth.SharesBurnt
    shape("ee473f96486a2f4b93ccb6729f121223e96975db6e0d6a5ef01f56477e3eab3b", 2, 32, Part::Topic1), // steth.ExternalSharesMinted
    shape("ad21467656c56eb8c99f7916faa12f7a657a04d8802100acec62e92451ac5606", 1, 32, Part::Caller), // steth.ExternalSharesBurnt
    shape("4ee34277c93491eeca655ad5c42ae1c193a5719e1c8837df9058af7696817cce", 1, 32, Part::Context), // steth.ExternalEtherTransferredToBuffer
    shape("4e80196ef1285462b2c4ee20f88e18e58c59e405eec6fd51f5fd1d614bc98a7f", 1, 32, Part::Context), // steth.ExternalBadDebtInternalized
    shape("13c514ee70ee403f89bdf5ab83908edba92a77e3a61e19b774670c0a2cb2d7e6", 1, 32, Part::Context), // steth.MaxExternalRatioBpSet
    shape("1252331d4f3ee8a9f0a3484c4c2fb059c70a047b5dc5482a3ee6415f742d9f2e", 2, 64, Part::Context), // steth.ClValidatorsUpdated
    shape("e0aacfc334457703148118055ec794ac17654c6f918d29638ba3b18003cee5ff", 1, 32, Part::Context), // steth.DepositedValidatorsChanged
    shape("92dd3cb149a1eebd51fd8c2a3653fd96f30c4ac01d4f850fc16d46abd6c3e92f", 2, 160, Part::Context), // steth.EthDistributed
    shape("af00d86be4cd299db16aa59803992e174fa88b67d81a0c7dd0148f9a75606a8d", 2, 96, Part::Context), // steth.InternalShareRateUpdated
    shape("26d1807b479eaba249c1214b82e4b65bbb0cc73ee8a17901324b1ef1b5904e49", 1, 0, Part::Context), // steth.StakingPaused
    shape("edaeeae9aed70c4545d3ab0065713261c9cee8d6cf5c8b07f52f0a65fd91efda", 1, 0, Part::Context), // steth.StakingResumed
    shape("ce9fddf6179affa1ea7bf36d80a6bf0284e0f3b91f4b2fa6eea2af923e7fac2d", 1, 64, Part::Context), // steth.StakingLimitSet
    shape("9b2a687c198898fcc32a33bbc610d478f177a73ab7352023e6cc1de5bf12a3df", 1, 0, Part::Context), // steth.StakingLimitRemoved
    shape("d27f9b0c98bdee27044afa149eadcd2047d6399cb6613a45c5b87e6aca76e6b5", 1, 32, Part::Context), // steth.ElRewardsReceived
    shape("6e5086f7e1ab04bd826e77faae35b1bcfe31bd144623361a40ea4af51670b1c3", 1, 32, Part::Context), // steth.WithdrawalsReceived
    shape("61f9416d3c29deb4e424342445a2b132738430becd9fa275e11297c90668b22e", 1, 32, Part::Context), // steth.LidoLocatorSet
    shape("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef", 3, 32, Part::Topics12), // erc20.Transfer
    shape("8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925", 3, 32, Part::Topics12), // erc20.Approval
];

fn insert(out: &mut BTreeMap<Addr, BTreeSet<Addr>>, contract: &[u8], holder: &[u8]) {
    if let (Ok(contract), Ok(holder)) = (Addr::try_from(contract), Addr::try_from(holder)) {
        if contract != ZERO && holder != ZERO {
            out.entry(contract).or_default().insert(holder);
        }
    }
}
fn visit(out: &mut BTreeMap<Addr, BTreeSet<Addr>>, from: &[u8], log: &eth::Log, caller: Option<&[u8]>) {
    let Some(topic0) = log.topics.first() else {
        return;
    };
    let mut matched = false;
    for s in SHAPES.iter().filter(|s| s.topic0[..] == topic0[..]) {
        if log.topics.len() != s.topics || log.data.len() != s.data || log.topics[1..].iter().any(|t| t.len() < 32) {
            continue;
        }
        let topic = |i: usize| Some(&log.topics[i][12..32]);
        let word = |i: usize| Some(&log.data[i * 32 + 12..i * 32 + 32]);
        let parts = match s.part {
            Part::RpcOwner => continue,
            Part::Caller => match caller {
                Some(caller) => [Some(caller), None],
                None => continue,
            },
            Part::Context => [None, None],
            Part::Topic1 => [topic(1), None],
            Part::Topic2 => [topic(2), None],
            Part::Topics12 => [topic(1), topic(2)],
            Part::Word0 => [word(0), None],
            Part::Words01 => [word(0), word(1)],
        };
        matched = true;
        for holder in parts.into_iter().flatten() {
            insert(out, &log.address, holder);
        }
    }
    if matched {
        insert(out, &log.address, from);
        insert(out, &log.address, &log.address);
    }
}
/// The RPC reference's `(contract, holder)` candidates for a block, by
/// contract. Successful transactions' logs from non-reverted calls (receipt
/// logs when a trace has no calls) that match a reference event shape add
/// its holders, `tx.from` and the contract; zero or non-20-byte addresses are
/// dropped. USDT `Issue`/`Redeem` are omitted: their holder needs `owner()` RPC.
pub(crate) fn candidates(block: &eth::Block) -> BTreeMap<Addr, BTreeSet<Addr>> {
    let mut out = BTreeMap::new();
    for tx in block.transaction_traces.iter().filter(|tx| tx.status == SUCCEEDED) {
        if tx.calls.is_empty() {
            for log in tx.receipt.iter().flat_map(|r| &r.logs) {
                visit(&mut out, &tx.from, log, None);
            }
        }
        for call in tx.calls.iter().filter(|c| !c.state_reverted) {
            for log in &call.logs {
                visit(&mut out, &tx.from, log, Some(&call.caller));
            }
        }
    }
    out
}

/// Exact signed sum of uint256 words, as wrapping 320-bit two's complement.
/// A block cannot hold 2^63 words, so equal residues are equal integers.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct Net([u64; 5]);
fn limb(w: &Word, i: usize) -> u64 {
    if i < 4 {
        u64::from_be_bytes(w[24 - 8 * i..32 - 8 * i].try_into().unwrap())
    } else {
        0
    }
}
impl Net {
    fn add(&mut self, w: &Word) {
        self.merge(&Net([limb(w, 0), limb(w, 1), limb(w, 2), limb(w, 3), 0]));
    }
    fn sub(&mut self, w: &Word) {
        let mut borrow = false;
        for i in 0..5 {
            let (a, b1) = self.0[i].overflowing_sub(limb(w, i));
            let (b, b2) = a.overflowing_sub(u64::from(borrow));
            self.0[i] = b;
            borrow = b1 || b2;
        }
    }
    fn merge(&mut self, other: &Net) {
        let mut carry = false;
        for i in 0..5 {
            let (a, c1) = self.0[i].overflowing_add(other.0[i]);
            let (b, c2) = a.overflowing_add(u64::from(carry));
            self.0[i] = b;
            carry = c1 || c2;
        }
    }
    fn is_zero(&self) -> bool {
        self.0 == [0; 5]
    }
    /// The magnitude, little-endian limbs.
    fn abs(&self) -> [u64; 5] {
        if self.0[4] >> 63 == 0 {
            return self.0;
        }
        let mut out = [0; 5];
        let mut carry = true;
        for (o, l) in out.iter_mut().zip(self.0) {
            let (v, c) = (!l).overflowing_add(u64::from(carry));
            *o = v;
            carry = c;
        }
        out
    }
}
/// `|delta| * 1000 >= |flow|`: a mismatch too large for a counter.
fn large(flow: &Net, delta: &Net) -> bool {
    let (flow, delta) = (flow.abs(), delta.abs());
    let mut scaled = [0u64; 5];
    let mut carry = 0u128;
    for (s, d) in scaled.iter_mut().zip(delta) {
        let p = u128::from(d) * 1000 + carry;
        *s = p as u64;
        carry = p >> 64;
    }
    carry > 0 || scaled.iter().rev().cmp(flow.iter().rev()).is_ge()
}

fn word(bytes: &[u8]) -> Option<Word> {
    let mut out = [0; 32];
    out.get_mut(32usize.checked_sub(bytes.len())?..)?.copy_from_slice(bytes);
    Some(out)
}
fn topic_address(t: &[u8]) -> Option<Addr> {
    (t.len() == 32 && t[..12] == [0; 12]).then(|| t[12..].try_into().unwrap())
}
/// The verified 64-byte Keccak preimage of `key` in a frame's hex
/// `hash -> preimage` map. Only 128-character values can hold 64 bytes, so
/// longer recorded preimages are never decoded.
fn preimage64(call: &eth::Call, key: &Word) -> Option<[u8; 64]> {
    let value = call.keccak_preimages.get(&hex::encode(key)).filter(|v| v.len() == 128)?;
    let bytes: [u8; 64] = hex::decode(value).ok()?.try_into().ok()?;
    (hash(&bytes) == *key).then_some(bytes)
}
/// The verified same-frame preimage `pad(holder) || base` of a storage key.
fn preimage(call: &eth::Call, key: &Word) -> Option<(Addr, Word)> {
    let bytes = preimage64(call, key)?;
    (bytes[..12] == [0; 12]).then(|| (bytes[12..32].try_into().unwrap(), bytes[32..].try_into().unwrap()))
}

struct Write {
    frame: Frame,
    key: Word,
    old: Word,
    new: Word,
    ordinal: u64,
}
/// Persisted storage writes of candidate contracts with their frame, from the
/// shared persistence rules.
struct Persisted<'a> {
    tokens: &'a BTreeMap<Addr, BTreeSet<Addr>>,
    writes: BTreeMap<Addr, Vec<Write>>,
    doubt: BTreeSet<Addr>,
}
impl Persisted<'_> {
    fn token(&self, address: &[u8]) -> Option<Addr> {
        Addr::try_from(address).ok().filter(|a| self.tokens.contains_key(a))
    }
}
impl persist::Sink for Persisted<'_> {
    fn storage(&mut self, c: &eth::StorageChange, ctx: persist::Ctx) {
        let Some(address) = self.token(&c.address) else {
            return;
        };
        match (word(&c.key), word(&c.old_value), word(&c.new_value)) {
            (Some(key), Some(old), Some(new)) if c.ordinal > 0 => self.writes.entry(address).or_default().push(Write {
                frame: (ctx.scope == persist::Scope::SystemCall, ctx.tx_index, ctx.call_index),
                key,
                old,
                new,
                ordinal: c.ordinal,
            }),
            _ => {
                self.doubt.insert(address);
            }
        }
    }
    fn balance(&mut self, _: &eth::BalanceChange, _: persist::Ctx) {}
    fn nonce(&mut self, _: &eth::NonceChange, _: persist::Ctx) {}
    /// Removed code: a contract created and destroyed in one transaction.
    fn code(&mut self, c: &eth::CodeChange, _: persist::Ctx) {
        if let Some(address) = self.token(&c.address).filter(|_| c.new_code.is_empty()) {
            self.doubt.insert(address);
        }
    }
}

/// Net `Transfer` flows per frame of contracts with persisted writes, and the
/// holders that their balance events name. Self-destructing contracts and
/// malformed event addresses are doubted.
fn flows<'a>(block: &'a eth::Block, writes: &BTreeMap<Addr, Vec<Write>>, doubt: &mut BTreeSet<Addr>) -> (Flows<'a>, Named) {
    let mut flows = Flows::new();
    let mut named = Named::new();
    for tx in block.transaction_traces.iter().filter(|tx| tx.status == SUCCEEDED) {
        for call in tx.calls.iter().filter(|c| !c.state_reverted) {
            if call.suicide {
                // SELFDESTRUCT acts on the storage context: a DELEGATECALL's caller.
                let context = if call.call_type == DELEGATE { &call.caller } else { &call.address };
                if let Ok(address) = Addr::try_from(context.as_slice()) {
                    doubt.insert(address);
                }
            }
            let frame = (false, tx.index, call.index);
            let mut nets = BTreeMap::<Addr, BTreeMap<Addr, Net>>::new();
            for log in &call.logs {
                let Some(token) = Addr::try_from(log.address.as_slice()).ok().filter(|t| writes.contains_key(t)) else {
                    continue;
                };
                let (Some(t0), Ok(value)) = (log.topics.first(), Word::try_from(log.data.as_slice())) else {
                    continue;
                };
                let transfer = log.topics.len() == 3 && t0[..] == TRANSFER;
                let credit_debit = log.topics.len() == 2 && CREDITS_DEBITS.iter().any(|t| t[..] == t0[..]);
                if !(transfer || credit_debit) {
                    continue;
                }
                let Some(holders) = log.topics[1..].iter().map(|t| topic_address(t)).collect::<Option<Vec<_>>>() else {
                    doubt.insert(token);
                    continue;
                };
                named.entry(token).or_default().extend(holders.iter().map(|h| (frame, *h)));
                if transfer && holders[0] != holders[1] {
                    let net = nets.entry(token).or_default();
                    if holders[0] != ZERO {
                        net.entry(holders[0]).or_default().sub(&value);
                    }
                    if holders[1] != ZERO {
                        net.entry(holders[1]).or_default().add(&value);
                    }
                }
            }
            for (token, mut net) in nets {
                net.retain(|_, n| !n.is_zero());
                if !net.is_empty() {
                    flows.insert((frame, token), (call, net));
                }
            }
        }
    }
    (flows, named)
}

/// Votes for one mapping base of one contract in one block.
#[derive(Clone, Copy, Default)]
struct Tally {
    /// Holder sides whose net persisted delta equals their net event flow;
    /// flows below 10^6 count only while the base has no mismatch.
    exact: u32,
    /// Holder sides with a nonzero flow and a different delta.
    mismatch: u32,
    /// Mismatches whose delta is at least a thousandth of the flow.
    large: u32,
    /// Exact votes for flows below 10^6.
    dust: u32,
    /// The base has its own verified 64-byte preimage: an inner mapping.
    nested: bool,
}
fn votes(flows: &Flows, writes: &BTreeMap<Addr, Vec<Write>>) -> BTreeMap<Addr, BTreeMap<Word, Tally>> {
    // Net persisted delta per key, per frame and contract.
    let mut framed = BTreeMap::<(Frame, Addr), BTreeMap<Word, Net>>::new();
    for (token, ws) in writes {
        for w in ws.iter().filter(|w| flows.contains_key(&(w.frame, *token))) {
            let d = framed.entry((w.frame, *token)).or_default().entry(w.key).or_default();
            d.add(&w.new);
            d.sub(&w.old);
        }
    }
    let mut tallies = BTreeMap::<Addr, BTreeMap<Word, Tally>>::new();
    for (key, (call, net)) in flows {
        // Each distinct key, and each base, is resolved once per frame.
        let mut deltas = BTreeMap::<(Addr, Word), Net>::new();
        for (k, d) in framed.get(key).into_iter().flatten() {
            if let Some(resolved) = preimage(call, k) {
                deltas.entry(resolved).or_default().merge(d);
            }
        }
        let mut nested = BTreeMap::<Word, bool>::new();
        let t = tallies.entry(key.1).or_default();
        for ((holder, base), delta) in deltas {
            let Some(flow) = net.get(&holder) else {
                continue;
            };
            let tally = t.entry(base).or_default();
            tally.nested |= *nested.entry(base).or_insert_with(|| preimage64(call, &base).is_some());
            if *flow == delta {
                tally.exact += 1;
                let f = flow.abs();
                tally.dust += u32::from(f[1..] == [0; 4] && f[0] < 1_000_000);
            } else {
                tally.mismatch += 1;
                tally.large += u32::from(large(flow, &delta));
            }
        }
    }
    // A small exact match cannot tell a stored balance from a scaled one.
    for tally in tallies.values_mut().flat_map(|t| t.values_mut()).filter(|t| t.mismatch > 0) {
        tally.exact -= tally.dust;
    }
    tallies
}
/// The base every traced `balanceOf` reads, if it has exact votes, or else
/// the base with the most exact votes; a tie chooses none. Inner mappings
/// (allowances) and bases with more large mismatches than exact votes (a
/// packed word whose other field moves) are never chosen.
fn choose(tallies: &BTreeMap<Word, Tally>, reads: &[Read]) -> Option<Word> {
    let plain = |t: &Tally| t.exact > 0 && !t.nested;
    let base = 'base: {
        if let Some(base) = reads.first().and_then(|r| r.bases.first()) {
            if reads.iter().all(|r| r.pure(base)) && tallies.get(base).is_some_and(plain) {
                break 'base *base;
            }
        }
        let mut best: Option<(Word, u32)> = None;
        let mut tied = false;
        for (base, t) in tallies.iter().filter(|(_, t)| plain(t)) {
            match best {
                Some((_, n)) if n > t.exact => {}
                Some((_, n)) if n == t.exact => tied = true,
                _ => {
                    best = Some((*base, t.exact));
                    tied = false;
                }
            }
        }
        best.filter(|_| !tied)?.0
    };
    (tallies[&base].large <= tallies[&base].exact).then_some(base)
}

/// A traced `balanceOf(holder)` call to a contract.
struct Read {
    holder: Addr,
    begin: u64,
    end: u64,
    value: Word,
    /// In a non-reverted frame: the value reflects persisted state.
    persisted: bool,
    /// Up to two bases of the verified `pad(holder) || base` preimages
    /// computed in the read's storage context (the frame and its DELEGATECALL
    /// descendants).
    bases: Vec<Word>,
    /// A call out of that storage context.
    external: bool,
}
impl Read {
    fn scan(&mut self, call: &eth::Call) {
        let holder = hex::encode(self.holder);
        for (key, value) in &call.keccak_preimages {
            // Match the hex text of `pad(holder)` before decoding and hashing.
            let v = value.as_bytes();
            if self.bases.len() == 2 || v.len() != 128 || v[..24].iter().any(|c| *c != b'0') || !v[24..64].eq_ignore_ascii_case(holder.as_bytes()) {
                continue;
            }
            let (Ok(bytes), Ok(key)) = (hex::decode(v), hex::decode(key)) else {
                continue;
            };
            let base: Word = bytes[32..].try_into().unwrap();
            if hash(&bytes)[..] == key[..] && !self.bases.contains(&base) {
                self.bases.push(base);
            }
        }
    }
    /// The read computed exactly `pad(holder) || base` and called nothing else.
    fn pure(&self, base: &Word) -> bool {
        self.bases == [*base] && !self.external
    }
}
fn is_read(call: &eth::Call) -> bool {
    !call.status_failed
        && (call.call_type == CALL || call.call_type == STATIC)
        && call.input.len() == 36
        && call.input[..4] == BALANCE_OF
        && call.input[4..16] == [0; 12]
        && call.return_data.len() == 32
        && call.begin_ordinal != 0
}
/// Traced `balanceOf(address)` calls of voted contracts: a completed CALL or
/// STATICCALL to the contract in a successful transaction, selector
/// `0x70a08231`, a 36-byte input and a 32-byte return.
fn reads<T>(block: &eth::Block, tokens: &BTreeMap<Addr, T>) -> BTreeMap<Addr, Vec<Read>> {
    let mut out = BTreeMap::<Addr, Vec<Read>>::new();
    let token = |c: &eth::Call| Addr::try_from(c.address.as_slice()).ok().filter(|t| tokens.contains_key(t));
    for tx in block.transaction_traces.iter().filter(|tx| tx.status == SUCCEEDED) {
        if !tx.calls.iter().any(|c| is_read(c) && token(c).is_some()) {
            continue;
        }
        // One pass assigns every call its storage context (itself, or its
        // parent's for a DELEGATECALL) and groups each context's DELEGATECALL
        // members and the other calls made from it.
        let mut context = BTreeMap::<u32, u32>::new();
        let (mut members, mut outside) = (BTreeMap::<u32, Vec<&eth::Call>>::new(), BTreeMap::<u32, Vec<&eth::Call>>::new());
        for c in &tx.calls {
            let parent = context.get(&c.parent_index).copied();
            let own = match parent {
                Some(p) if c.call_type == DELEGATE => {
                    members.entry(p).or_default().push(c);
                    p
                }
                _ => {
                    if let Some(p) = parent {
                        outside.entry(p).or_default().push(c);
                    }
                    c.index
                }
            };
            context.insert(c.index, own);
        }
        for call in tx.calls.iter().filter(|c| is_read(c)) {
            let Some(token) = token(call) else {
                continue;
            };
            let mut read = Read {
                holder: call.input[16..].try_into().unwrap(),
                begin: call.begin_ordinal,
                end: call.end_ordinal,
                value: call.return_data[..].try_into().unwrap(),
                persisted: !call.state_reverted,
                bases: Vec::new(),
                external: false,
            };
            read.scan(call);
            let members = members.get(&call.index).map(Vec::as_slice).unwrap_or_default();
            for c in members {
                read.scan(c);
            }
            // A call out of the storage context counts unless it only returns
            // the address that its parent then DELEGATECALLs: a beacon lookup.
            let delegates: BTreeSet<_> = members.iter().map(|c| (c.parent_index, c.address.as_slice())).collect();
            read.external = outside.get(&call.index).into_iter().flatten().any(|c| {
                let resolved = (c.return_data.len() == 32 && c.return_data[..12] == [0; 12]).then(|| &c.return_data[12..]);
                resolved.is_none_or(|a| !delegates.contains(&(c.parent_index, a)))
            });
            out.entry(token).or_default().push(read);
        }
    }
    out
}

/// The end-of-block balances of one voted contract, or `None` on any doubt.
fn decide(
    token: &Addr,
    tallies: &BTreeMap<Word, Tally>,
    writes: &[Write],
    reads: &[Read],
    holders: &BTreeSet<Addr>,
    named: &BTreeSet<(Frame, Addr)>,
) -> Option<BTreeMap<Addr, Word>> {
    let base = choose(tallies, reads)?;
    let mut by_key = BTreeMap::<Word, Vec<&Write>>::new();
    for w in writes {
        by_key.entry(w.key).or_default().push(w);
    }
    // Persisted writes to one key must chain in execution order.
    if by_key
        .values()
        .any(|ws| ws.windows(2).any(|p| p[1].ordinal <= p[0].ordinal || p[1].old != p[0].new))
    {
        return None;
    }
    if reads.iter().any(|r| !r.pure(&base)) {
        return None;
    }
    let read: BTreeSet<Addr> = reads.iter().filter(|r| r.persisted).map(|r| r.holder).collect();
    // Other holder mappings that move with the event flows suggest a computed
    // balance. With two or more exact votes and no other exact mapping,
    // counter-sized mismatches are ignored.
    let exact = tallies[&base].exact;
    let others = || tallies.iter().filter(|(b, t)| **b != base && !t.nested).map(|(_, t)| t);
    let large_only = exact >= 2 && others().all(|t| t.exact == 0);
    let shadow: u32 = others().map(|t| if large_only { t.large } else { t.mismatch }).sum();
    // Then a pure `balanceOf` vouches only for its own holder's code path.
    let shadowed = shadow >= exact;
    let mut values = BTreeMap::new();
    for r in reads.iter().filter(|r| r.persisted) {
        match by_key.get(&mapping(&r.holder, &base)) {
            Some(ws) => {
                // Writes to the key are in increasing ordinal order.
                if ws.get(ws.partition_point(|w| w.ordinal <= r.begin)).is_some_and(|w| w.ordinal < r.end) {
                    continue;
                }
                let stored = ws[..ws.partition_point(|w| w.ordinal < r.begin)]
                    .last()
                    .map(|w| w.new)
                    .or_else(|| ws.get(ws.partition_point(|w| w.ordinal <= r.end)).map(|w| w.old));
                if stored.is_some_and(|s| s != r.value) {
                    return None;
                }
            }
            // The holder's mapping word has no persisted write in the block.
            None if holders.contains(&r.holder) => {
                if values.insert(r.holder, r.value).is_some_and(|v| v != r.value) {
                    return None;
                }
            }
            None => {}
        }
    }
    for holder in holders {
        if let Some(last) = by_key.get(&mapping(holder, &base)).and_then(|ws| ws.last()) {
            // A third party's word changed with no balance event naming it in
            // that frame (a flag, an admin or a sync write) needs its own read.
            if holder == token || named.contains(&(last.frame, *holder)) || read.contains(holder) {
                values.insert(*holder, last.new);
            }
        }
    }
    if shadowed {
        values.retain(|holder, _| read.contains(holder));
    }
    Some(values)
}

/// Inferred end-of-block balances, by `(contract, holder)`.
pub(crate) fn rows(block: &eth::Block) -> BTreeMap<(Addr, Addr), Word> {
    let mut out = BTreeMap::new();
    let candidates = candidates(block);
    if candidates.is_empty() {
        return out;
    }
    let mut persisted = Persisted {
        tokens: &candidates,
        writes: BTreeMap::new(),
        doubt: BTreeSet::new(),
    };
    if persist::collect_block(block, &mut persisted).is_err() {
        return out;
    }
    let Persisted { mut writes, mut doubt, .. } = persisted;
    for ws in writes.values_mut() {
        ws.sort_by_key(|w| w.ordinal);
    }
    let (flows, named) = flows(block, &writes, &mut doubt);
    let tallies = votes(&flows, &writes);
    let reads = reads(block, &tallies);
    let nobody = BTreeSet::new();
    // A contract has tallies only if a nonzero `Transfer` moved a holder.
    for (token, t) in tallies.iter().filter(|(token, _)| !doubt.contains(*token)) {
        let token_reads = reads.get(token).map(Vec::as_slice).unwrap_or_default();
        let named = named.get(token).unwrap_or(&nobody);
        if let Some(values) = decide(token, t, &writes[token], token_reads, &candidates[token], named) {
            out.extend(values.into_iter().map(|(holder, value)| ((*token, holder), value)));
        }
    }
    out
}
