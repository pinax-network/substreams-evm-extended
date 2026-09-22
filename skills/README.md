# Skills

Procedural guides for continuing this repository's roadmap. Each directory
holds one `SKILL.md` with frontmatter (`name`, `description`) so it can be
dropped into `.claude/skills/` or read by a person. They complement the
normative rules in [`../AGENTS.md`](../AGENTS.md) and the state record in
[`../docs/handoff.md`](../docs/handoff.md); where they disagree, `AGENTS.md`
wins.

| Skill | Use it when |
| --- | --- |
| [`balance-state-package`](balance-state-package/SKILL.md) | adding or changing a protocol balance-state map (`evm.balance_state.v1`): structure, fail-closed rules, fixtures, tests, README, review checklist |
| [`live-qualification`](live-qualification/SKILL.md) | checking a packaged map against the live chain: credentials in the environment, runtime-epoch binding from upgrade writes, bounded runs, same-block getter parity, evidence and claims |
| [`offline-evidence-replay`](offline-evidence-replay/SKILL.md) | producing evidence without RPC: saved-block inventory, trimming single-transaction fixtures, replay tools, evidence JSON and claim wording |
| [`pinned-source-research`](pinned-source-research/SKILL.md) | binding a contract to a pinned source: what to extract, how to record pins, packed words, keccak-named and ERC-7201 slots |
| [`roadmap-issue-workflow`](roadmap-issue-workflow/SKILL.md) | taking a GitHub issue from acceptance list to merged PR and status comment, including CI mechanics |
| [`storage-layout-verification`](storage-layout-verification/SKILL.md) | upgrading inferred slots to compiler-verified layouts with `solc --storage-layout` |
