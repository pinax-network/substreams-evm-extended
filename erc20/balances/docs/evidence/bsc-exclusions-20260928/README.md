# Offline evidence artifact locations

`report.json` is copied unchanged from the fresh local diagnostic output
`out/bsc-exclusions-20260928/run-6` using a fresh worktree-local Cargo target
directory. Its artifact names refer to the generated
output directory. In the committed tree:

- `report.json`, `blocks.json`, and `unresolved.json` are in this directory.
- `cases.json`, `take-candidate-NOT-QUALIFIED.json`, `take-source.json`,
  `tops-source.json`, and the three `.pb` transaction fixtures are under
  [tests/fixtures/bsc-exclusions-20260928](../../../tests/fixtures/bsc-exclusions-20260928/README.md).
- `take-emitted-rows.json` remains in ignored `out/`; its SHA-256 is in the report.
  The Rust diagnostic regenerates it from the saved, SHA-256-bound input blocks
  and canonical reference. The full original blocks/reference remain ignored
  cache inputs, not newly downloaded data.

`attempts.md` describes the unchanged earlier attempt reports. These artifacts
are offline evidence only; `qualified` remains false and no historical package
or profile digest is replaced.
