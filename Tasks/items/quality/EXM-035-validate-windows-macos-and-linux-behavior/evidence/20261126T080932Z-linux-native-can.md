# EXM-035 Linux native-feature and virtual CAN runtime

Source 58f5eb938998e6ee9d5dffd4f5f430be2c7da3b6. Rust 1.98.0,
committed Cargo.lock, verified Zig 0.15.2 x86_64-linux-gnu.2.35 wrapper.
Actual Ubuntu 24.04.5 LTS x64, kernel 6.8.0-147-generic, QEMU 11.1.0 q35
TCG, 4 vCPUs/4 GiB RAM, synthetic writable overlay, no physical passthrough.
Tool/image acquisition identities are in the preceding handoff evidence.

```text
cargo +1.98.0 test -p exmon-capture --features native-can,native-hid,native-usb,native-usbmon --target x86_64-unknown-linux-gnu --locked --no-run --message-format=json
node target/exm035-linux/guest.mjs --script target/exm035-linux/guest-native.sh
```

All 49 ordinary tests passed across 11 executable suites, each using
--test-threads=1. All guest executable hashes matched the manifest and the
runner emitted EXMON_NATIVE_TESTS_PASS. Five explicit opt-in device tests
were ignored in that ordinary run; these are not counted as passes.

The subsequent opt-in SocketCAN test initially could not run because the
minimal cloud kernel did not include vcan. Its version-matched official
Ubuntu linux-modules-extra-6.8.0-147-generic_6.8.0-147.147_amd64.deb was
fetched over HTTPS and checked against apt-get --print-uris download's
SHA-512 from the guest's authenticated APT indexes:
db40a805db2c8a76e111f6f67b09840942a1076a2746b5b8f0819977d7a56ce48e26fa88ad38bde01120de7cd32176d34b6f6d50ba84585bdc78516a76cefe6b.

Trusted 7-Zip extracted the package. Only vcan.ko and its can-dev.ko dependency
were transferred to the guest. The initial vcan insmod diagnosed the missing
can_dropped_invalid_skb symbol; modinfo reported can-dev. After loading that
version-matched dependency, the same prepared test ran successfully:

```text
EXMON_TEST_VCAN=exmoncan0 ./native-tests/can --ignored --exact native_vcan_rx_tx_and_filtering --test-threads=1
```

The test passed (one test, 0.26 seconds) and EXMON_NATIVE_VCAN_PASS was printed.
ip -details -json confirmed link kind vcan before the test; the test independently
verifies that virtual kind before opening sockets or transmitting. The guest-only
link and the two newly added modules were removed afterward. No physical CAN,
HID or USB device was present or used. Remaining ignored HID/usbmon tests need
their declared emulated fixtures; this result does not qualify them. Native
Ethernet/libpcap was not enabled in this run. Hardware timing is EXM-036.

## Executed artifacts

| Target | SHA-256 |
| --- | --- |
| exmon_capture | 5862cf6a1bbaf13b9d2546a1df207b19a1d770bafbae78c05b724097ba3ee34b |
| usb_transfers | 4944ff2546d139f951b7e211f0ef0c917b44dc329fbee3d8f4256dd6807fafee |
| serial | 5ee1dea2ed4dadb514b9086f44cfef705b6574f3a14a36e0f0b8614dfb7e070c |
| ethernet | 1a83472fd1b1e2d2944e3be255dc40722cfa575050d7b4044a12951f4684a309 |
| hid | 38776944352b2fd1ce36abc71fa96b10588e8694d5fdde1414be97c806661931 |
| transmit | bf09a863c053f68acce6c062f19fabd79540c95e3b205400cbf382d4cc361978 |
| can | 87f97ae2361a4dbb943f3dfce54012bef967cd4d9838511c13ee312ab4ccd607 |
| synthetic | 21e24ba22feeb24975e59ae8bef184bae93ca3ac074655bc867aefb85b43c9a2 |
| sockets | 8e7cd07160b3c9769ec27a4b392d684c71275e949c3280d19c4ceaec35ab8460 |
| usbmon | 93410840950998618f7b4c96d067fbd98b365a35f480a127daada953c87da10b |
| adapter | f0eebed997fa91cdb047dfd764b01459e303217fe3c52d7c6819c3b16f10d677 |

## Local evidence hashes

- `native-build.jsonl`: `3d141db747d3cf69dd3631b5f05af4570042a149148d94e4ee3f3548ce46e377`
- `native-binaries.json`: `34b6fc2a438ccf33c5b1adb79cf102f36dc90dd211e0eac6983c8374ac6c4346`
- `native-runtime.log`: `ac1de773ff0eb851a91aebe66d58a3ffba74abff592df0b7478299f7e9c0b9d9`
- `module-uri.log`: `1e6c6d6341b437d50cbc90101a4dec1e009f104908e9d71f8abf2135ed08d429`
- `vcan-runtime.log`: `c7802a7d81a6496ee441a81dec8ea2730baf388233bd757c72524373b8a03558`
- `vcan-with-dependency-runtime.log`: `7e904aa90ccc35fe10593051400d4205416ea7940013d90beb40fc3b2ae53ac9`
- `vcan.ko`: `bffe39eb1e57d68c73f27ccb1d17cf211b32c710355cb29010d6e82b52f0d4ac`
- `can-dev.ko`: `02141ccabaa6c3bf8525e310426d01f028ee5a1c53df24013c1b258466f1873c`
- `guest-native.sh`: `dd6ce57d9e0f1e25bf98f55923305766f6e5bd89ad68bd4969182e95fa2e455e`
- `guest-vcan-with-dependency.sh`: `24ab46ee700745dcc16357c3e713a34956dbbc1bec21fff89f5234924754e41f`

The bounded synthetic toolcache/evidence remain ignored. No binary, guest disk,
module, credential or generated build output is committed.
