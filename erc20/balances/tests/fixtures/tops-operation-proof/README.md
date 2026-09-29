# TOPS host proof inputs — NOT QUALIFIED

See [the focused proof](../../../docs/tops-operation-proof.md) for scope,
source provenance, limits and reproduction. This directory contains no production
layout or admission candidate.

- `capture.json`: complete original direct-token capture, SHA-256
  `bed5d7155189f849140fce44c4971a526a139b40b1ac4cc7d83db48b057a4c1a`.
- `settings.json`, `original-input.json`, `full-input.json`: exact original input
  and declared outputSelection-only extension. Fresh full compiler output
  `full-output.json` has SHA-256
  `653ee0d4f9ac33899d6bcc828f44e18eba517e02882bf09b39114fd3b0d73f7d`.
- `TOPSCleanup.sol`, extraction record and separate harness input/output: exact
  selected function bodies, padded roots 31/32 and one added wrapper. Full output
  SHA-256 `d250f4f254f7bb50f8c1fb25995e6ba9d154278f510b1ffbfb7858d339b58956`.
  This harness is source semantics only, not the original runtime.
- `primary-sources.json` contains eight exact official OpenZeppelin bodies and
  immutable URLs. Custom token source retains the explicit independent-pin gap.
- Compiler manifest/version and both original/fresh metadata strings are retained.
  Both complete bytecodes preserve compiler CBOR; no tail normalization occurs.
- `execution-spec-block.py` and `LICENSE-execution-specs` preserve the immutable
  primary TIMESTAMP specification and CC0 notice. OpenZeppelin MIT license is
  separate. Original token and generated harness retain `Unlicensed` notices.

Final compiler binding is `out/tops-operation-proof-20260929/source-04`; the
`operations-07` matrix passed 605 calls (505 returns, 98 expected reverts and two
explicit unsupported boundaries). Earlier source/failed operation attempts remain
in ignored output and are never replaced by these selected fixture copies.
