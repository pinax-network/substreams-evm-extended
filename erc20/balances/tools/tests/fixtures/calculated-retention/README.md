# Closed calculated-retention source captures

These four complete JSON files are byte-identical copies of the original saved source captures. The historical LBP and reflection checkpoints, getter expectations and controls remain at their original paths; the retention module imports and hashes them without rewriting them.

| Capture | Address | SHA-256 |
| --- | --- | --- |
| LBP | `0x88886f0fd371dff856291badced45922bc888888` | `def766d9a5568fd07f95ea88cb8dfade485d452addd0a2698c495032b9cff2dd` |
| hLBP | `0x5e3cbc82d020be91a989eb747934104e9ab585fe` | `5b61fc47bb9852f0a362309d06cd638c3a6c3540d1c6a2ad5cb778df736d0d8b` |
| BabyDoge | `0xc748673057861a797275cd8a068abb95a902e8de` | `a83222287f471061fb345c52e4e3565591fb1e7253a554dab65fccf3ea4356dd` |
| 10SET | `0x1f64fdad335ed784898effb5ce22d54d8f432523` | `083a44ab01fd109dad4e7a4b7c5a4061b963454a86a35d9f0be439a045c8d87d` |

Original source labels, full source sets, compiler settings, metadata, layout, creation/runtime bytes and declared transformations are preserved. Runtime reconstruction applies 79 immutable substitutions for LBP, 17 for hLBP, eight immutable and one CBOR substitution for BabyDoge, and none for 10SET. No creation execution, new compiler output, primary-source refresh or deployment qualification is claimed.

See [the host retention contract](../../../../docs/calculated-retention.md) for finite-holder scope and provenance limits.

`journal.jsonl` is a NEW typed offline journal derived from all 1,024 hash-pinned Extended blocks in `[122288006,122289030)`. Its SHA-256 is `ebe020cca7b75e562cfe124eb074d275a2c8eb9f69f667099e98e9da3a88876d`. The final replay independently regenerates these bytes; the portable Rust regression compares the original 923 getter observations and exact snapshot/undo state without requiring the ignored PB cache. It contains persisted raw storage/code effects and full applied clocks, never expected getter amounts.
