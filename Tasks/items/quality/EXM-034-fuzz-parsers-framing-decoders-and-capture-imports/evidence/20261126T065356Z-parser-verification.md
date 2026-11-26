# EXM-034 parser verification

Published base ef12eef844d81dc42c0741a71f48b0424e5e71d2, tested source tree 3658b03201a5cea492ea8053634698ac02876b0c.
FDTasks policy SHA-256 b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8.
Windows Server 2025 x64 reference VPS, Rust/Cargo 1.98.0, eight logical CPUs.

The supervised final command was
`powershell -NoProfile -File scripts/run-parser-campaign.ps1 -OutputDirectory target/exm034-final`.
It passed with exit 0, 60.307 wall seconds, 57.797 CPU seconds and 2,134,016 peak
sampled private bytes, within the predeclared 100 s / 90 CPU s / 256 MiB limits.
Executable SHA-256 b08b82e1c7bf30c6ba80b81ec273ed87a7b5e3333331c1425de4370f7d499269.

| Family | Duration | Cases |
| --- | ---: | ---: |
| Network decoding | 15.000 s | 12,528,679 |
| PCAP / PCAPNG | 15.000 s | 3,395,455 |
| Six framing modes | 15.002 s | 136,733 |
| Definitions / codec / checksums | 15.000 s | 155,508 |

AC-01 passed. No crash or violated assertion was discovered, so no crash regression
was required. Failures retain family, case and seed index for deterministic replay.
Inputs come from the synthetic corpus and committed boundary seeds. The generator
uses bounded xorshift mutation, not coverage instrumentation. The finite campaign
provides evidence for these inputs and duration. Existing exhaustive split,
known-vector, malformed-container and exact codec tests cover the direct
contracts alongside the mutation runner.

AC-02 passed. The final smoke command
`cargo +1.98.0 test -p exmon-app --test parser_campaign --locked`
passed its deterministic successful decode/edit/checksum and bounded parser checks.
The manual campaign test is ignored by default and was explicitly run by the supervisor.
`cargo +1.98.0 test -p exmon-protocol --test network --test framing --test fields --test codec --locked`
passed 50 tests. `cargo +1.98.0 test -p exmon-storage --test pcap --locked`
passed 12 tests. Format, workspace strict Clippy, architecture and staged diff
checks passed. Initial Clippy findings in the new harness were corrected before
the final campaign. Product runtime sources were unchanged.

Final output hashes before cleanup follow.

- campaign.log 730664ae8f00c3a6d35817f9ba0d57d3a319da55c66c026d84b3733be005335a
- summary.json 6711a5ac245b6253ab345166dc57c8ac807a32ed78baf1cd06c6ee0a6213323e
- Empty errors.log e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855

Owned campaign output was cleaned. Source and normal build cache remain.
No physical interface, platform qualification or release is claimed.
