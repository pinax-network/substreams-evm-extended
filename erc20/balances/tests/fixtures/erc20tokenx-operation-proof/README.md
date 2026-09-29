# ERC20TokenX Phase A fixtures

Complete original ORI/FNA captures are copied byte for byte from the historical
source-review cache. Their common sources/settings regenerate with exact
solc 0.7.5 (optimizer 200, default Istanbul), yielding all 7,896 runtime bytes and
9,347 creation bytes without substitution. Constructor arguments and deployment
records remain distinct and unmodified.

PHI has only the original source-request miss plus a separate historical runtime
file/report from block 122288067. Its full runtime equals ORI/FNA; no PHI source,
creation, or deployment record is invented. The historical report is not fresh
qualification.

`primary-sources.json` retains all four exact OZ 3.4.2 dependency bodies and marks
the custom token's independent primary revision unestablished. The pinned MIT
license and original source notices remain. Compiler input differs from the
original only by outputSelection. Full output/manifest/version are separately
bound; host code and tests reject mutations to the complete records.

See [scope and limits](../../../docs/erc20tokenx-operation-proof.md). No ingestion
candidate is contained here. Runtime operation evidence is synthetic; the
unchanged VM cannot complete the constructor's CHAINID path.
