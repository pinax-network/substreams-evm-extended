# PTokenV2 operation proof inputs

These are original capture and fresh compiler artifacts for a **host-only,
NOT-QUALIFIED Phase A proof**. There is no candidate layout file here.

- `capture.json`: complete original selected source/metadata/layout/runtime and
  creation capture; SHA-256 `14d5dcc50d8dd1b04fc6283e0d1fafad66fd7fe0bfafdff224a9ba6eda203f9c`.
- `compiler-input-original.json`: captured compiler input serialized as JSON.
- `compiler-input.json`: same input with only documented `outputSelection` added.
- `compiler-output.json`: full fresh official 0.8.28 output; SHA-256
  `c85b99ca52a3ef59a6aeeefe10afd2c7c95ab852754293ab16b1489e199da891`.
- `primary-sources.json`: twenty exact immutable OpenZeppelin dependency sources,
  URLs and hashes. The token's independent public repository remains unresolved.
- `solc-list.json` / `compiler-version.txt`: official manifest association and
  executed compiler version. The compiler binary remains in the local output
  directory and is checked against its manifest SHA before execution.
- `LICENSE-OZ`: exact upstream MIT license. Captured source SPDX notices remain
  embedded in the complete source inputs; no Solidity body is rewritten.

Runtime and creation code are byte-for-byte equal to the captured arrays; no
metadata stripping, normalization, imports relocation, links or immutables are
allowed. The only creation append is the independently checked 256-byte captured
constructor ABI. Saved raw match labels remain `match`.

See [the bounded proof and qualification limits](../../../docs/ptoken-operation-proof.md).
