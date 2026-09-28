# BTR operation proof inputs

These complete source/compiler artifacts support a **host-only, NOT-QUALIFIED
Phase A proof**. They are not candidate layout parameters.

- `implementation-capture.json`: all 31 original source files, metadata, layout,
  creation/runtime bytes and bindings; SHA-256
  `b45110e73361a6bdaf50181ec265b0c9cc00cab6dbafbe1c76f1262a57139b21`.
- `proxy-capture.json`: all eight original source files and complete equivalent
  bindings; SHA-256
  `9c9fcd09b032a0ac632041217f5ce4000a20492c555e30e5ca5896fb14b6b320`.
- Each `*-compiler-input-original.json` is the captured input serialized as JSON.
  Its `*-compiler-input.json` adds only documented `outputSelection` fields.
- `implementation-compiler-output.json`: complete fresh solc 0.8.24 output,
  SHA-256 `c72916720a301b5eedf94f100b9c7cca4e35000eb77a252d5af68e1d7c53fcec`.
- `proxy-compiler-output.json`: complete fresh solc 0.8.24 output, SHA-256
  `6d93cd857f4bf15d98f3ed89bbfa154bf399957cd1f0df406ac4bc94f6b98a05`.
- `primary-sources.json`: 38 independently fetched immutable OpenZeppelin
  dependency bodies, paths, URLs and hashes. The custom token's exact independent
  primary repository remains unresolved and is explicitly recorded.
- `solc-list.json` and `compiler-version.txt`: official immutable release
  manifest and actual compiler version. The binary stays in local output and
  must pass its pinned SHA-256 and Keccak checks before execution.
- `LICENSE-implementation` and `LICENSE-proxy`: exact upstream MIT licenses.
  Original source SPDX notices remain embedded, including token `UNLICENSED`.

Both original match labels remain `exact_match`. Recompilation preserves all
sources and settings; no imports, source text or metadata are normalized. The
implementation's five declared self immutables are independently reconstructed;
the proxy has none. Its 416-byte constructor append is independently encoded and
bound in full. The proxy bytecode is compiled and compared, not dispatched.

Source-01 is the preserved initial successful compilation with an unfinished
case matrix. Final `source-02` reproduces every fixture artifact byte-for-byte;
its complete compiler reports and the strengthened 910-call `operations-03`
execution are linked in the
[focused proof and limits](../../../docs/btr-operation-proof.md).
