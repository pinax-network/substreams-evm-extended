# Selected SecuritiesToken source proof fixture

The complete original capture is preserved in `capture.json`. It contains all
31 literal sources and their notices, exact standard input, saved compiler
artifacts and the full saved runtime. On-chain creation bytes, creation match
and all deployment fields are null; they are never reconstructed as chain facts.

`compiler-input-original.json` and `compiler-input.json` differ only by the
explicit output selection needed for complete artifacts/generated source 31.
The official solc 0.8.24 release manifest, version response, complete fresh
output and upstream license files are retained. Runtime bytes and saved/fresh
compiler creation objects are compared in full without transformation.

`primary-sources.json` binds every captured path and hash. Twenty OpenZeppelin
v5.3.0 bodies and three BEP-677 interfaces match exact immutable public sources.
Three BEP counterpart bodies differ and are preserved as such; five BUSL-1.1
token/compliance/pause sources have no recovered exact public revision.
Exact URL/body/hash checks prevent those eight gaps from becoming matches.

`metadata-saved.json` contains literal Unicode and `metadata-fresh.json` uses
the official compiler's escaped serialization. Both raw strings are separately
SHA256-pinned and their complete parsed content is equal. Neither source nor
metadata is normalized. The first strict-string failure is preserved in the
ignored attempt directory.

The [focused proof note](../../../docs/securities-operation-proof.md) records
scope and final evidence. This fixture is a host test input, not an ingestion
layout, a deployed-state attestation or admission for the seventeen profiles.
