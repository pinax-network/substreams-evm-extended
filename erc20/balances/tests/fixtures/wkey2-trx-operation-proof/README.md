# wkeyDAO2 / TRX Phase A fixtures

Both complete original captures retain their exact raw bytes and original
`match` labels. The five wkeyDAO2 sources include four exact full OZ 3.4.2
files and one custom AGPL source with an unestablished independent origin.
TRX is one original CRLF flattened file: only its whole AccessControl and
EnumerableSet declarations match the pinned upstream after CRLF-to-LF
comparison. The whole file's origin/license is not inferred from those matches.

Official solc 0.7.5 and 0.6.6 reproduce complete original compiler code/maps and
raw metadata with only outputSelection added. Each captured runtime/creation
requires its one exact 53-byte CBOR substitution; creation retains 32/79 bytes
after that span and independent 96/192-byte constructor appends. No byte is
trimmed or normalized for compilation. Full captures, input/output, metadata,
primary records, notices, license and official compiler manifest/version are
independently bound by the host helper.

See [scope and limits](../../../docs/wkeydao2-trx-operation-proof.md).
These fixtures add no ingestion candidate. Both constructors encounter
unsupported CHAINID; synthetic runtime prestate does not establish deployed
initialization, coherent role sets or signature/external-call behavior.

Final source-03 and operations-03 use one identical frozen source inventory.
All 1,230 compiler/captured case pairs agree. Four exact TRX generated storage
instruction sites lack source text and are identified separately; no Solidity
location is invented. See the linked report for final counts and preserved
preliminary/failure evidence.
