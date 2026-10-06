//! `erc20/balances` keeps its embedded copy of the persistence rules, and
//! `common/persist` must not diverge from it (AGENTS.md): their code before
//! `#[cfg(test)]` must be equal, comment and blank lines aside.

/// The lines of a persistence-rules source before its tests, without
/// comments or blank lines.
fn code(source: &str) -> Vec<&str> {
    source
        .lines()
        .take_while(|line| line.trim() != "#[cfg(test)]")
        .map(str::trim_end)
        .filter(|line| !line.is_empty() && !line.trim_start().starts_with("//"))
        .collect()
}

#[test]
fn embedded_persistence_rules_equal_common_persist() {
    let embedded = code(include_str!("../src/persist.rs"));
    let shared = code(include_str!("../../../common/persist/src/lib.rs"));
    assert!(embedded.len() > 100, "the rules were not found");
    assert_eq!(embedded, shared);
}
