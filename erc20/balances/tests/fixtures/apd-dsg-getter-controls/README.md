# APD / DSG saved runtime getter controls

These six files are byte-identical copies of repository-root
`out/typed450-candidates/inputs/rank401` and `rank418`: complete Sourcify responses
and the saved runtime files at 122288005 and 122289029. They preserve the original
`match` labels, compiler inputs/outputs, source bodies, settings, source maps,
metadata and transformation records. No compilation or network request was made.

APD's flattened source declares AGPL-3.0; DSG's flattened source declares
AGPL-3.0-or-later AND MIT and retains the embedded notices. These original notices
remain intact inside the captures. An exact independent public maintainer revision
for either whole token source is not established by this proof; no dependency
primary pin is inferred merely from a flattened filename or comment.

The Rust host verifier pins the full raw responses before parsing and reconstructs
all runtime and creation bytes using only the exact reviewed transformations.
APD requires seven 32-byte immutable replacements and a 53-byte CBOR replacement;
DSG requires only the CBOR replacement. Creation bindings also preserve APD's
64-byte constructor append and DSG's 288-byte append, including DSG's intervening
32-byte creation constant after CBOR. Binding creation bytes does not execute or
qualify either deployment.

Both saved outputs omit AST and generated source bodies. The host tool retains
original source-map spans and explicitly labels missing source IDs if encountered.
No fresh source body is invented. The measured getter/metadata matrix resolves
all effects to the captured Solidity source; that does not establish complete
attribution for other paths.

The original typed450 protobufs, canonical expectations and historical DSG
operation records remain in their existing directories and are independently
pinned by the new verifier and source inventory. They are not rewritten here.
The existing retained-ledger tests from PR #93 remain separate.

See [the focused proof document](../../../docs/apd-dsg-getter-controls.md) for
measured counts, evidence links and remaining qualification boundaries.
