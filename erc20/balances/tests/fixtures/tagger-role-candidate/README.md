# Tagger exact role paths — NOT-QUALIFIED

This separate candidate covers only BSC Tagger
`0x208bf3e7da9639f1eaefa2de78c23396b0682025`. It replaces the baseline's
root-6 width-two rule with `[bytes32]`, offset 1, width 1 for the outer admin
word and `[bytes32,address]`, offset 0, width 1 for membership. All other
profile fields, the 431-profile baseline and qualified 425 cohort are unchanged.

`TaggerToken.json` preserves the complete original saved capture, SHA-256
`5fac5c544be56adfb7779efabf5a23d435fcb7de04c37af0c6de2e1700fb5663`.
Its sole flattened `TaggerToken.sol` file has SHA-256
`a986e195f98b68a206a95aa0b591c5b527a0186fb62d4a5238430fc30f20e0eb`,
including original line endings. No independent public token commit was
recovered. Its OpenZeppelin-like sections are not independently verified
separate upstream dependency files. This primary-source gap remains open.

The dedicated host verifier binds the raw capture before parsing, complete
source/input/metadata/source-ID/output sets, ABI and complete storage layout.
The compiler is `0.8.20+commit.a1b79de6`, optimizer enabled with 200 runs,
no libraries and no explicit input EVM target; the saved compiler metadata
resolves the default to Shanghai. It is a reconstruction of saved compiler
output, not a fresh compilation or deployed-code measurement.

The original `match`, `runtimeMatch` and `creationMatch` labels remain `match`,
not `exact_match`. Runtime (11,535 bytes) and creation (13,809 bytes) each
require exactly one recorded 53-byte CBOR replacement, at offsets 11,482 and
13,692 respectively. `source-review.json` pins both before/after values.
There are no immutable or library substitutions and no constructor arguments.
The creation suffix after the replaced block is 64 bytes and must remain
unchanged; neither code array is stripped or truncated. Whole reconstructed
runtime Keccak is
`0x949e3cc8727e6f0a8cd2734af1a44ce06cbd279749923868cad8b5bbe7dfd996`;
creation Keccak is
`0x56c86488af76ad8890b7365a0d008ba8ef28e588a617e75453093ac688457caf`.
Point/Bedrock's no-transformation verifier remains unchanged.

In the complete source, `RoleData.members` is the inner address-to-bool mapping
at offset 0; `adminRole` is the outer bytes32 word at offset 1. The two
constructor assignments establish deployer/operator admin roles. Public
`onlyOwner setRoleAdmin(bytes32,bytes32)` admits arbitrary role/admin values.
`BasicAccessControl._checkRole` also permits the owner without membership,
so inherited grant/revoke accepts arbitrary roles under those checks;
renounce still requires the account to equal the caller. These are source
reachability observations, not executed authorization or current-owner proof.

The inherited `balanceOf` reads balances at root 0. Other storage remains:
allowances 1, supply/name/symbol/status 2–5, owner 7, signer 8, mode 9,
position/lock IDs 10–11, claims 12, fee collectors 13 and launched 14.
There is no stored enumerable role set or array in this layout.

Synthetic Extended-record controls exercise both role paths with simultaneous
balances, arbitrary admin values, grant/revoke/renounce/no-op shapes,
unchanged metadata, unknown adjacent/deeper paths, malformed words, failed or
reverted records and runtime changes. They do not execute Solidity, enforce
role authorization or prove producer visibility. Saved replay and remaining
promotion gates are documented in [the candidate report](../../../docs/tagger-role-candidate.md).
