# YBC raw inputs

The seven original JSON captures are byte-for-byte historical copies, pinned by
`tools/src/ybc_retention/binding.rs`. `parent-storage.json` and
`final-storage.json` contain 1,705 independently read raw words each at
122288005 and 122289029. `historical.json` contains the separate getter
expectations; its decoded states never initialize the retained ledger.
`overrides.json` preserves all 30 original raw state-override maps and independent
responses. Overrides are separate synthetic branches of the parent checkpoint.

`token-source.json` preserves the complete flattened source, notices, compiler
input/output/layout, original match labels and exact CBOR transformation.
`prestate.json` supplies all three original runtime byte arrays; `calls.json`
preserves the 968 reviewed external calls. The reward helper source remains
unavailable. No new primary origin, compilation, source execution or live
qualification is claimed.

The new finite key registry can include names from both raw checkpoints. Only
parent values initialize the parent ledger. Moving a cursor neither prunes old
known hours nor initializes new hours from the end capture. The two checkpoints
are independent observations, not a consecutive-block sequence. A new saved-PB
journal is generated separately by `replay_ybc_retention`.

`journal.jsonl` contains the 1,024 selected persisted-effect records decoded from
the original PB window [122288006,122289030), with all twelve holder identities
supplied by the independent parent checkpoint. It records raw effects and clocks,
not expected balances. `original-block-manifest.json` is the unchanged captured
PB digest manifest. The portable journal test checks those identities, continuation,
restart and bounded undo; the replay driver requires byte-for-byte regeneration
from the original PBs. Neither file initializes missing hourly words.
