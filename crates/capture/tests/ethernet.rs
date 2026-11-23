use exmon_capture::{adapter::*, ethernet::*};
use exmon_domain::{Direction, observation::Metadata};
use std::sync::{Arc, Mutex};

#[cfg(all(target_os = "windows", feature = "native-pcap"))]
mod native_windows {
    use super::*;
    use exmon_capture::native_pcap::NativePcap;
    use std::{
        process::Command,
        sync::atomic::{AtomicBool, Ordering},
        thread::{self, JoinHandle},
        time::{Duration, Instant},
    };

    fn fixture_name() -> String {
        assert_eq!(
            std::env::var("EXMON_TEST_WINDOWS_ETHERNET").as_deref(),
            Ok("km-test")
        );
        // Only the explicitly provisioned root-enumerated test device is eligible.
        let output=Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command", "Get-CimInstance Win32_NetworkAdapter | Where-Object { $_.PNPDeviceID -eq 'ROOT\\DEVGEN\\EXMON_ETHERNET_TEST' } | Select-Object GUID,ProductName,ServiceName | ConvertTo-Json -Compress"]).output().unwrap();
        assert!(output.status.success());
        let info: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            info.is_object(),
            "exactly one explicitly created test adapter required"
        );
        assert_eq!(info["ProductName"], "Microsoft KM-TEST Loopback Adapter");
        assert_eq!(info["ServiceName"], "kmloop");
        let guid = info["GUID"].as_str().unwrap();
        let inner = guid
            .strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .unwrap();
        assert_eq!(inner.len(), 36);
        assert!(
            inner
                .bytes()
                .enumerate()
                .all(|(i, b)| if [8, 13, 18, 23].contains(&i) {
                    b == b'-'
                } else {
                    b.is_ascii_hexdigit()
                })
        );
        format!("\\Device\\NPF_{guid}")
    }

    struct Generator {
        stop: Arc<AtomicBool>,
        handle: Option<JoinHandle<()>>,
    }
    impl Generator {
        fn finish(mut self) {
            self.stop.store(true, Ordering::Release);
            self.handle.take().unwrap().join().unwrap();
        }
    }
    impl Drop for Generator {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    #[test]
    #[ignore = "requires installed Npcap and explicit Windows virtual fixture opt-in"]
    fn native_npcap_loopback_remains_unsupported_by_ethernet_contract() {
        fixture_name();
        let mut adapter = Ethernet::new(NativePcap::default(), Options::default()).unwrap();
        let interfaces = adapter.enumerate(64).unwrap();
        let id = interfaces
            .iter()
            .find(|d| adapter.interface_name(d.id) == Some("\\Device\\NPF_Loopback"))
            .expect("Npcap loopback not discovered")
            .id;
        adapter.open(id).unwrap();
        adapter.configure(64).unwrap();
        assert_eq!(adapter.start(), Err(Error::Unsupported));
        assert_eq!(
            adapter.poll(&Cancellation::default(), 8, 512).err(),
            Some(Error::InvalidState)
        );
        adapter.close().unwrap();
    }

    #[test]
    #[ignore = "requires installed Npcap and explicitly provisioned Microsoft KM-TEST adapter; no physical interface"]
    fn native_km_test_fidelity_filter_snapshot_and_stop_under_load() {
        let name = fixture_name();
        let mut expected = vec![2, 0, 0, 0, 0, 1, 2, 0, 0, 0, 0, 2, 0x88, 0xb5];
        expected.extend(14..64);
        for snapshot in [64, 16] {
            let mut adapter = Ethernet::new(
                NativePcap::default(),
                Options {
                    promiscuous: true,
                    timeout_ms: 10,
                    filter: "ether proto 0x88b5".into(),
                },
            )
            .unwrap();
            let interfaces = adapter.enumerate(64).unwrap();
            let id = interfaces
                .iter()
                .find(|d| {
                    adapter
                        .interface_name(d.id)
                        .is_some_and(|s| s.eq_ignore_ascii_case(&name))
                })
                .expect("selected virtual adapter not discovered")
                .id;
            adapter.open(id).unwrap();
            adapter.configure(snapshot).unwrap();
            adapter.start().unwrap();
            assert_eq!(adapter.transmit(&expected), Err(Error::Unsupported));
            let stop = Arc::new(AtomicBool::new(false));
            let finished = Arc::new(AtomicBool::new(false));
            let (worker_stop, worker_finished) = (stop.clone(), finished.clone());
            let (worker_name, mut frame) = (name.clone(), expected.clone());
            let generator = Generator {
                stop,
                handle: Some(thread::spawn(move || {
                    let mut injector = pcap::Capture::from_device(worker_name.as_str())
                        .unwrap()
                        .promisc(false)
                        .timeout(10)
                        .open()
                        .unwrap();
                    assert_eq!(injector.get_datalink(), pcap::Linktype::ETHERNET);
                    for i in 0..4000 {
                        if worker_stop.load(Ordering::Acquire) {
                            break;
                        }
                        frame[13] = if i % 2 == 0 { 0xb6 } else { 0xb5 };
                        injector.sendpacket(frame.as_slice()).unwrap();
                        thread::sleep(Duration::from_millis(1));
                    }
                    worker_finished.store(true, Ordering::Release);
                })),
            };
            let deadline = Instant::now() + Duration::from_secs(3);
            let mut count = 0;
            while count < 32 && Instant::now() < deadline {
                let batch = adapter.poll(&Cancellation::default(), 8, 512).unwrap();
                assert!(batch.len() <= 8);
                for record in batch {
                    assert_eq!(record.bytes(), &expected[..snapshot]);
                    assert_eq!(record.original_length(), Some(64));
                    assert!(matches!(
                        record.metadata(),
                        Metadata::Ethernet { link_type: 1 }
                    ));
                    assert_eq!(record.direction(), Direction::Unknown);
                    assert!(record.source_timestamp_ns().unwrap() > 0);
                    count += 1;
                }
                thread::sleep(Duration::from_millis(1));
            }
            assert!(
                count >= 32,
                "native Windows capture did not receive expected filtered frames"
            );
            assert!(
                !finished.load(Ordering::Acquire),
                "generator ended before stop-under-load check"
            );
            let cancel = Cancellation::default();
            let begin = Instant::now();
            cancel.cancel();
            assert_eq!(adapter.poll(&cancel, 8, 512).err(), Some(Error::Cancelled));
            adapter.stop().unwrap();
            assert!(
                begin.elapsed() < Duration::from_millis(500),
                "native Windows cancellation/close exceeded budget"
            );
            assert_eq!(
                adapter.poll(&Cancellation::default(), 8, 512).err(),
                Some(Error::InvalidState)
            );
            adapter.close().unwrap();
            generator.finish();
        }
    }
}

#[cfg(all(target_os = "linux", feature = "native-pcap"))]
mod native_virtual {
    use super::*;
    use exmon_capture::native_pcap::NativePcap;
    use std::{
        fs,
        process::{Child, Command, Stdio},
        time::{Duration, Instant},
    };

    fn fixture() {
        assert_eq!(
            std::env::var("EXMON_TEST_ETHERNET").as_deref(),
            Ok("qemu-veth")
        );
        for (file, expected) in [
            ("product_name", "EXMON-NET-FIXTURE"),
            ("sys_vendor", "EXMON-QEMU"),
        ] {
            assert_eq!(
                fs::read_to_string(format!("/sys/class/dmi/id/{file}"))
                    .unwrap()
                    .trim(),
                expected
            );
        }
        for name in ["exmonrx", "exmontx"] {
            let output = Command::new("ip")
                .args(["-details", "-json", "link", "show", "dev", name])
                .output()
                .unwrap();
            assert!(output.status.success());
            let link: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(
                link[0]["linkinfo"]["info_kind"], "veth",
                "refuse non-virtual interface before opening capture or generating traffic"
            );
        }
    }

    fn selected(snapshot: usize) -> Ethernet<NativePcap> {
        let mut adapter = Ethernet::new(
            NativePcap::default(),
            Options {
                promiscuous: true,
                timeout_ms: 10,
                filter: "ether proto 0x88b5".into(),
            },
        )
        .unwrap();
        let interfaces = adapter.enumerate(64).unwrap();
        let id = interfaces
            .iter()
            .find(|d| adapter.interface_name(d.id) == Some("exmonrx"))
            .unwrap()
            .id;
        adapter.open(id).unwrap();
        adapter.configure(snapshot).unwrap();
        adapter
    }

    struct Generator(Child);
    impl Drop for Generator {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    #[ignore = "requires isolated EXMON QEMU veth fixture; never physical Ethernet"]
    fn native_veth_fidelity_filter_snapshot_and_stop_under_load() {
        fixture();
        let mut bytes = vec![2, 0, 0, 0, 0, 1, 2, 0, 0, 0, 0, 2, 0x88, 0xb5];
        bytes.extend(14..64);
        let script = format!(
            "import socket,time\ns=socket.socket(socket.AF_PACKET,socket.SOCK_RAW)\ns.settimeout(1)\ns.bind(('exmontx',0))\nf=bytearray({bytes:?})\ndeadline=time.monotonic()+4\ni=0\nwhile time.monotonic()<deadline:\n f[13]=0xb5 if i%2 else 0xb6\n s.send(f)\n i+=1\n time.sleep(0.0002)\n"
        );
        for snapshot in [64, 16] {
            let mut adapter = selected(snapshot);
            adapter.start().unwrap();
            assert_eq!(adapter.transmit(&bytes), Err(Error::Unsupported));
            let mut generator = Generator(
                Command::new("python3")
                    .args(["-c", &script])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut count = 0;
            while count < 32 && Instant::now() < deadline {
                let batch = adapter.poll(&Cancellation::default(), 8, 512).unwrap();
                assert!(batch.len() <= 8);
                for observation in batch {
                    assert_eq!(observation.bytes(), &bytes[..snapshot]);
                    assert_eq!(observation.original_length(), Some(64));
                    assert_eq!(observation.direction(), Direction::Unknown);
                    assert!(matches!(
                        observation.metadata(),
                        Metadata::Ethernet { link_type: 1 }
                    ));
                    assert!(observation.source_timestamp_ns().unwrap() > 0);
                    count += 1;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(
                count >= 32,
                "native capture did not receive expected filtered traffic"
            );
            assert!(
                generator.0.try_wait().unwrap().is_none(),
                "generator ended before stop-under-load check"
            );
            let cancellation = Cancellation::default();
            let stop = Instant::now();
            cancellation.cancel();
            assert_eq!(
                adapter.poll(&cancellation, 8, 512).err(),
                Some(Error::Cancelled)
            );
            adapter.stop().unwrap();
            assert!(
                stop.elapsed() < Duration::from_millis(500),
                "native cancel/close exceeded fixture stop budget"
            );
            assert_eq!(
                adapter.poll(&Cancellation::default(), 8, 512).err(),
                Some(Error::InvalidState)
            );
            adapter.close().unwrap();
            drop(generator);
        }
    }

    #[test]
    #[ignore = "requires unprivileged account in isolated EXMON QEMU veth fixture"]
    fn native_veth_permission_denied_is_explicit() {
        fixture();
        let uid = Command::new("id").arg("-u").output().unwrap();
        assert!(uid.status.success());
        assert_ne!(String::from_utf8(uid.stdout).unwrap().trim(), "0");
        let mut adapter = selected(64);
        assert_eq!(adapter.start(), Err(Error::PermissionDenied));
        assert_eq!(
            adapter.poll(&Cancellation::default(), 8, 512).err(),
            Some(Error::InvalidState)
        );
        adapter.close().unwrap();
    }
}

#[derive(Default)]
struct Control {
    active: bool,
    activates: usize,
    reads: usize,
    snapshot: usize,
    promisc: bool,
    filter: String,
    link: u32,
    denied: bool,
}
struct Fixture(Arc<Mutex<Control>>);
fn frame() -> Vec<u8> {
    // Synthetic locally administered addresses, experimental EtherType 0x88B5.
    vec![2, 0, 0, 0, 0, 1, 2, 0, 0, 0, 0, 2, 0x88, 0xb5, 0xa5, 0, 42]
}
impl Source for Fixture {
    fn list(&mut self, maximum: usize) -> Result<Vec<String>, Error> {
        Ok(vec!["synthetic-ethernet".into()]
            .into_iter()
            .take(maximum)
            .collect())
    }
    fn activate(&mut self, name: &str, snapshot: usize, options: &Options) -> Result<u32, Error> {
        assert_eq!(name, "synthetic-ethernet");
        let mut c = self.0.lock().unwrap();
        if c.denied {
            return Err(Error::PermissionDenied);
        }
        c.active = true;
        c.activates += 1;
        c.snapshot = snapshot;
        c.promisc = options.promiscuous;
        c.filter = options.filter.clone();
        Ok(c.link)
    }
    fn next(&mut self) -> Result<Option<RawFrame>, Error> {
        let mut c = self.0.lock().unwrap();
        assert!(c.active);
        c.reads += 1;
        let full = frame();
        let bytes = full[..full.len().min(c.snapshot)].to_vec();
        Ok(Some(RawFrame {
            timestamp_ns: 123_000 + c.reads as u64,
            original_length: full.len(),
            bytes,
        }))
    }
    fn close(&mut self) {
        self.0.lock().unwrap().active = false;
    }
}
fn setup(snapshot: usize) -> (Ethernet<Fixture>, Arc<Mutex<Control>>) {
    let control = Arc::new(Mutex::new(Control {
        link: 1,
        ..Control::default()
    }));
    let mut adapter = Ethernet::new(
        Fixture(control.clone()),
        Options {
            promiscuous: true,
            timeout_ms: 20,
            filter: "ether proto 0x88b5".into(),
        },
    )
    .unwrap();
    let descriptors = adapter.enumerate(64).unwrap();
    assert_eq!(descriptors.len(), 1);
    assert_eq!(descriptors[0].tx, Availability::Unsupported);
    assert_eq!(control.lock().unwrap().activates, 0);
    adapter.open(0).unwrap();
    adapter.configure(snapshot).unwrap();
    assert_eq!(control.lock().unwrap().activates, 0);
    (adapter, control)
}

#[test]
fn exact_bytes_capture_time_truncation_and_options() {
    for snapshot in [12, 17] {
        let (mut adapter, control) = setup(snapshot);
        adapter.start().unwrap();
        let batch = adapter.poll(&Cancellation::default(), 4, 64).unwrap();
        assert_eq!(batch.len(), (64 / snapshot).min(4));
        assert_eq!(batch[0].bytes(), &frame()[..snapshot]);
        assert_eq!(batch[0].original_length(), Some(17));
        assert_eq!(batch[0].source_timestamp_ns(), Some(123001));
        assert_eq!(batch[0].direction(), Direction::Unknown);
        assert_eq!(batch[0].metadata(), &Metadata::Ethernet { link_type: 1 });
        let c = control.lock().unwrap();
        assert!(c.promisc);
        assert_eq!(c.filter, "ether proto 0x88b5");
    }
}

#[test]
fn stop_under_synthetic_load_is_finite_and_does_not_read_after_cancel() {
    let (mut adapter, control) = setup(17);
    adapter.start().unwrap();
    let token = Cancellation::default();
    for _ in 0..100 {
        assert_eq!(adapter.poll(&token, 4, 68).unwrap().len(), 4);
    }
    token.cancel();
    let before = control.lock().unwrap().reads;
    assert_eq!(adapter.poll(&token, 4, 68).err(), Some(Error::Cancelled));
    assert_eq!(control.lock().unwrap().reads, before);
    adapter.stop().unwrap();
    assert!(!control.lock().unwrap().active);
    assert_eq!(adapter.poll(&token, 4, 68).err(), Some(Error::InvalidState));
}

#[test]
fn unsupported_links_and_permission_denial_remain_explicit() {
    let (mut adapter, control) = setup(17);
    control.lock().unwrap().link = 113;
    assert_eq!(adapter.start(), Err(Error::Unsupported));
    assert!(!control.lock().unwrap().active);
    control.lock().unwrap().denied = true;
    assert_eq!(adapter.start(), Err(Error::PermissionDenied));
    assert!(
        Ethernet::new(
            Fixture(control),
            Options {
                timeout_ms: 0,
                ..Options::default()
            }
        )
        .is_err()
    );
}
