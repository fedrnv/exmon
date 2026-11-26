# Local parser mutation campaign

Run the deterministic smoke and supervised campaign on the Windows reference host.

```powershell
cargo +1.98.0 test -p exmon-app --test parser_campaign --locked
powershell -NoProfile -File scripts/run-parser-campaign.ps1 -OutputDirectory target/parser-campaign
```

Each of four families runs for at least 15 seconds and 256 cases. The supervisor
enforces 100 wall seconds, 90 CPU seconds and 256 MiB private memory. Inputs
derive from synthetic fixtures and explicit boundary seeds, with deterministic
xorshift mutations. No external capture files enter the corpus.

The campaign covers network payload bounds, bounded PCAP/PCAPNG reading, all six
framing modes with escaped stream inputs, and field/checksum codec admission,
editing and checksum repair. Smoke checks independently require a successful
decode, edit and checksum result. Protocol and capture integration regressions
remain part of task acceptance.

On a failure, retain the family, seed index and case from the error output.
Reconstruct the input with the committed seed and `mutate` function, minimize
the synthetic bytes while preserving the failure, and add a deterministic
regression before repeating the affected campaign. A successful finite campaign
establishes evidence for the tested inputs, duration and source revision.
