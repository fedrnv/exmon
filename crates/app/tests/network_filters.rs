#[path = "../../protocol/tests/support/filter_wire.rs"]
mod fixture;
use exmon_app::{
    network_filters::*,
    shell::{CaptureWorker, ShellError},
};
use exmon_capture::{
    adapter::{Adapter, Cancellation, Error},
    ethernet::{Options, RawFrame, Source},
};
#[cfg(feature = "native-pcap")]
use exmon_domain::Direction;
use exmon_domain::observation::TransportKind;
use fixture::*;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
struct Fake {
    frames: VecDeque<Vec<u8>>,
    log: Arc<Mutex<Vec<String>>>,
    validation: Result<(), Error>,
    fail_start: bool,
    next_error: Option<Error>,
}
impl Source for Fake {
    fn validate_filter(&mut self, filter: &str) -> Result<(), Error> {
        self.log.lock().unwrap().push(format!("validate:{filter}"));
        self.validation
    }
    fn list(&mut self, _: usize) -> Result<Vec<String>, Error> {
        self.log.lock().unwrap().push("list".into());
        Ok(vec!["test".into()])
    }
    fn activate(&mut self, _: &str, _: usize, options: &Options) -> Result<u32, Error> {
        self.log
            .lock()
            .unwrap()
            .push(format!("activate:{}", options.filter));
        if self.fail_start {
            Err(Error::Backend)
        } else {
            Ok(1)
        }
    }
    fn next(&mut self) -> Result<Option<RawFrame>, Error> {
        self.log.lock().unwrap().push("next".into());
        if let Some(error) = self.next_error {
            return Err(error);
        }
        Ok(self.frames.pop_front().map(|bytes| RawFrame {
            timestamp_ns: 99,
            original_length: bytes.len(),
            bytes,
        }))
    }
    fn close(&mut self) {
        self.log.lock().unwrap().push("close".into());
    }
}
fn fake(
    frames: Vec<Vec<u8>>,
    validation: Result<(), Error>,
    fail_start: bool,
) -> (Fake, Arc<Mutex<Vec<String>>>) {
    let log = Arc::new(Mutex::new(vec![]));
    (
        Fake {
            frames: frames.into(),
            log: log.clone(),
            validation,
            fail_start,
            next_error: None,
        },
        log,
    )
}
#[test]
fn filtered_adapter_installs_one_plan_and_post_fallback_has_same_results() {
    for validation in [Ok(()), Err(Error::Unsupported)] {
        let frames = vec![
            wire(false, true, 12345, 443, 0, false),
            wire(true, false, 12345, 443, 2, true),
            wire(false, false, 12345, 444, 0, false),
        ];
        let (source, log) = fake(frames.clone(), validation, false);
        let settings = NetworkFilter {
            interface: Some(0),
            ports: vec![443],
            ..Default::default()
        };
        let mut adapter = FilteredEthernet::new(source, settings, Options::default()).unwrap();
        assert!(!adapter.active());
        assert_eq!(
            adapter.mode(),
            if validation.is_ok() {
                FilterMode::BpfAndPost
            } else {
                FilterMode::PostOnly
            }
        );
        let bpf = adapter.plan().bpf().to_owned();
        assert_eq!(log.lock().unwrap().as_slice(), [format!("validate:{bpf}")]);
        adapter.enumerate(1).unwrap();
        assert_eq!(adapter.open(1), Err(Error::InvalidConfig));
        adapter.open(0).unwrap();
        adapter.configure(128).unwrap();
        adapter.start().unwrap();
        assert!(adapter.active());
        assert_eq!(
            log.lock().unwrap()[2],
            format!(
                "activate:{}",
                if validation.is_ok() { bpf.as_str() } else { "" }
            )
        );
        let received = adapter.poll(&Cancellation::default(), 8, 1024).unwrap();
        assert_eq!(received.len(), 2);
        assert_eq!(adapter.rejected(), 1);
        for (observation, expected) in received.iter().zip(&frames) {
            assert_eq!(observation.bytes(), expected);
            assert_eq!(observation.source_timestamp_ns(), Some(99));
        }
        adapter.stop().unwrap();
        assert!(!adapter.active());
        adapter.close().unwrap();
    }
}
#[test]
fn invalid_validation_and_activation_fail_closed_and_reject_all_poll_stays_finite() {
    let settings = NetworkFilter {
        ports: vec![443],
        ..Default::default()
    };
    let (source, log) = fake(vec![], Err(Error::InvalidConfig), false);
    assert!(matches!(
        FilteredEthernet::new(source, settings.clone(), Options::default()),
        Err(Error::InvalidConfig)
    ));
    assert_eq!(log.lock().unwrap().len(), 1);
    let (source, log) = fake(vec![], Ok(()), false);
    assert!(matches!(
        FilteredEthernet::new(
            source,
            NetworkFilter {
                ports: vec![1; 33],
                ..Default::default()
            },
            Options::default()
        ),
        Err(Error::InvalidConfig)
    ));
    assert!(log.lock().unwrap().is_empty());
    let (source, _) = fake(vec![], Ok(()), true);
    let mut adapter = FilteredEthernet::new(source, settings.clone(), Options::default()).unwrap();
    adapter.enumerate(1).unwrap();
    adapter.open(0).unwrap();
    adapter.configure(128).unwrap();
    assert_eq!(adapter.start(), Err(Error::Backend));
    assert!(!adapter.active());
    adapter.close().unwrap();
    let (source, log) = fake(
        vec![wire(false, true, 12345, 444, 0, false); 100],
        Err(Error::Unsupported),
        false,
    );
    let mut adapter = FilteredEthernet::new(source, settings, Options::default()).unwrap();
    adapter.enumerate(1).unwrap();
    adapter.open(0).unwrap();
    adapter.configure(128).unwrap();
    adapter.start().unwrap();
    assert!(
        adapter
            .poll(&Cancellation::default(), 4, 512)
            .unwrap()
            .is_empty()
    );
    assert_eq!(adapter.rejected(), 4);
    assert_eq!(
        log.lock()
            .unwrap()
            .iter()
            .filter(|s| s.as_str() == "next")
            .count(),
        4
    );
    let cancel = Cancellation::default();
    cancel.cancel();
    assert_eq!(adapter.poll(&cancel, 4, 512).err(), Some(Error::Cancelled));
    assert!(!adapter.active());
    adapter.close().unwrap();
}

#[test]
fn terminal_poll_failure_clears_effective_running_state() {
    let (mut source, log) = fake(vec![], Ok(()), false);
    source.next_error = Some(Error::Disconnected);
    let mut adapter =
        FilteredEthernet::new(source, NetworkFilter::default(), Options::default()).unwrap();
    adapter.enumerate(1).unwrap();
    adapter.open(0).unwrap();
    adapter.configure(128).unwrap();
    adapter.start().unwrap();
    assert!(adapter.active());
    assert_eq!(
        adapter.poll(&Cancellation::default(), 4, 512).err(),
        Some(Error::Disconnected)
    );
    assert!(!adapter.active());
    assert_eq!(log.lock().unwrap().last().unwrap(), "close");
    adapter.close().unwrap();
}
#[test]
fn actual_mock_worker_filters_off_thread_freezes_state_and_restarts_explicitly() {
    let mut worker = CaptureWorker::spawn().unwrap();
    let settings = NetworkFilter {
        ports: vec![12000],
        protocol: Some(6),
        ..Default::default()
    };
    worker
        .start_filtered(TransportKind::Tcp, 1, settings.clone())
        .unwrap();
    let accepted = worker.effective_filter().unwrap();
    assert_eq!(accepted.settings().ports, [12000]);
    assert_eq!(accepted.settings().interface, Some(1));
    assert_eq!(
        worker.start_filtered(TransportKind::Tcp, 1, NetworkFilter::default()),
        Err(ShellError::Busy)
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut count = 0;
    while count == 0 && Instant::now() < deadline {
        for batch in worker.drain(8) {
            for record in batch.records {
                assert!(accepted.matches(1, &record.observation));
                count += 1;
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(count > 0);
    assert_ne!(worker.worker_thread().unwrap(), std::thread::current().id());
    worker.stop();
    let deadline = Instant::now() + Duration::from_secs(1);
    while worker.status().running && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    worker.drain(8);
    worker
        .start_filtered(
            TransportKind::Tcp,
            2,
            NetworkFilter {
                ports: vec![12002],
                ..Default::default()
            },
        )
        .unwrap();
    let generation = worker.generation();
    let deadline = Instant::now() + Duration::from_secs(1);
    while worker.status().filtered_out == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(worker.status().filtered_out > 0);
    for batch in worker.drain(8) {
        if batch.generation == generation {
            assert!(batch.records.is_empty());
        }
    }
    assert_eq!(worker.effective_filter().unwrap().settings().ports, [12002]);
    worker.stop();
    assert_eq!(
        worker.start_filtered(TransportKind::Can, 1, settings),
        Err(ShellError::Invalid)
    );
}

#[test]
fn immediate_stop_restart_never_reports_an_old_generation_as_the_new_running_filter() {
    let worker = CaptureWorker::spawn().unwrap();
    for port in 12000..12024 {
        worker
            .start_filtered(
                TransportKind::Tcp,
                1,
                NetworkFilter {
                    ports: vec![port],
                    ..Default::default()
                },
            )
            .unwrap();
        let current = worker.generation();
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let status = worker.status();
            if status.running {
                assert_eq!(status.active_generation, Some(current));
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(worker.effective_filter().unwrap().settings().ports, [port]);
        worker.stop();
        // Deliberately do not wait for old backend teardown before the next Start.
    }
}

#[cfg(feature = "native-pcap")]
#[test]
fn actual_npcap_dead_compiler_and_interpreter_preserve_exact_postfilter_semantics() {
    use exmon_capture::native_pcap::NativePcap;
    let dead = pcap::Capture::dead(pcap::Linktype::ETHERNET).unwrap();
    let mut native = NativePcap::default();
    assert_eq!(
        native.validate_filter("ip and ("),
        Err(Error::InvalidConfig)
    );
    assert_eq!(
        native.activate(
            "must-not-open-an-interface",
            128,
            &Options {
                filter: "ip and (".into(),
                ..Default::default()
            }
        ),
        Err(Error::InvalidConfig)
    );
    let settings_list = vec![
        NetworkFilter {
            ports: vec![443],
            ..Default::default()
        },
        NetworkFilter {
            ports: vec![12345],
            protocol: Some(6),
            ..Default::default()
        },
        NetworkFilter {
            ports: vec![444],
            protocol: Some(17),
            ..Default::default()
        },
        NetworkFilter {
            source: Some("192.0.2.1".parse().unwrap()),
            ..Default::default()
        },
        NetworkFilter {
            destination: Some("198.51.100.2".parse().unwrap()),
            ports: vec![443],
            ..Default::default()
        },
        NetworkFilter {
            host: Some("192.0.2.1".parse().unwrap()),
            ..Default::default()
        },
        NetworkFilter {
            host: Some("2001:db8::2".parse().unwrap()),
            ports: vec![443],
            ..Default::default()
        },
        NetworkFilter {
            ports: (0..32).collect(),
            ..Default::default()
        },
    ];
    for settings in settings_list {
        let plan = FilterPlan::new(settings.clone()).unwrap();
        native.validate_filter(plan.bpf()).unwrap();
        let program = dead.compile(plan.bpf(), true).unwrap();
        let adapter =
            FilteredEthernet::new(NativePcap::default(), settings, Options::default()).unwrap();
        assert_eq!(adapter.mode(), FilterMode::BpfAndPost);
        assert!(!adapter.active());
        for v6 in [false, true] {
            for tcp in [false, true] {
                for tags in [0, 1, 2, 8] {
                    for extensions in [false, true] {
                        for port in [443, 444] {
                            let raw = wire(v6, tcp, 12345, port, tags, extensions);
                            for length in 0..=raw.len() {
                                let raw = &raw[..length];
                                let observation = observation(raw, Direction::Unknown);
                                let post = plan.matches(0, &observation);
                                let combined = program.filter(raw) && post;
                                assert_eq!(
                                    post,
                                    combined,
                                    "BPF dropped an accepted fixture: {}",
                                    plan.summary()
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(all(target_os = "windows", feature = "native-pcap"))]
#[test]
#[ignore = "requires explicit provisioned KM-TEST virtual adapter and Npcap; never physical"]
fn native_windows_filtered_ethernet_keeps_only_selected_ipv4_ipv6_ports() {
    use exmon_capture::native_pcap::NativePcap;
    use std::sync::atomic::{AtomicBool, Ordering};
    assert_eq!(
        std::env::var("EXMON_TEST_WINDOWS_ETHERNET").as_deref(),
        Ok("km-test")
    );
    // Exact root-enumerated fixture only. Forwarding must be disabled for both
    // families so synthetic IP traffic cannot be routed off this virtual NIC.
    let output=std::process::Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",
        "$a=@(Get-CimInstance Win32_NetworkAdapter | Where-Object { $_.PNPDeviceID -eq 'ROOT\\DEVGEN\\EXMON_ETHERNET_TEST' }); if ($a.Count -ne 1 -or $a[0].ProductName -ne 'Microsoft KM-TEST Loopback Adapter' -or $a[0].ServiceName -ne 'kmloop') { exit 2 }; $ip=@(Get-NetIPInterface -InterfaceIndex $a[0].InterfaceIndex); if ($ip.Count -ne 2 -or @($ip | Where-Object { $_.Forwarding -ne 0 }).Count -ne 0) { exit 3 }; $a[0].GUID"
    ]).output().unwrap();
    assert!(
        output.status.success(),
        "isolated virtual fixture guard failed"
    );
    let guid = String::from_utf8(output.stdout).unwrap();
    let guid = guid.trim();
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
    let name = format!("\\Device\\NPF_{guid}");
    let frames = [
        wire(false, true, 12345, 443, 0, false),
        wire(true, false, 12345, 443, 2, true),
        wire(false, false, 12345, 444, 0, false),
        wire(true, true, 12345, 444, 8, true),
    ];
    let mut capture = FilteredEthernet::new(
        NativePcap::default(),
        NetworkFilter {
            ports: vec![443],
            ..Default::default()
        },
        Options {
            promiscuous: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(capture.mode(), FilterMode::BpfAndPost);
    assert!(!capture.active());
    let selected = capture
        .enumerate(64)
        .unwrap()
        .into_iter()
        .find(|d| {
            capture
                .interface_name(d.id)
                .is_some_and(|n| n.eq_ignore_ascii_case(&name))
        })
        .expect("guarded virtual fixture missing")
        .id;
    capture.open(selected).unwrap();
    capture.configure(256).unwrap();
    capture.start().unwrap();
    assert!(capture.active());
    std::thread::scope(|scope| {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let sent_frames = &frames;
        let sender = scope.spawn(move || {
            let mut injector = pcap::Capture::from_device(name.as_str())
                .unwrap()
                .promisc(false)
                .timeout(10)
                .open()
                .unwrap();
            assert_eq!(injector.get_datalink(), pcap::Linktype::ETHERNET);
            for i in 0..1000 {
                if worker_stop.load(Ordering::Acquire) {
                    break;
                }
                injector.sendpacket(sent_frames[i % 4].as_slice()).unwrap();
                std::thread::sleep(Duration::from_millis(1));
            }
        });
        let mut counts = [0; 2];
        let deadline = Instant::now() + Duration::from_secs(3);
        while counts.iter().any(|n| *n < 16) && Instant::now() < deadline {
            for record in capture.poll(&Cancellation::default(), 8, 2048).unwrap() {
                let index = frames[..2]
                    .iter()
                    .position(|bytes| bytes == record.bytes())
                    .expect("nonmatching wire record escaped exact filter");
                counts[index] += 1;
                assert!(capture.plan().matches(selected, &record));
                assert_eq!(record.direction(), Direction::Unknown);
                assert_eq!(record.original_length(), Some(frames[index].len()));
                assert!(record.source_timestamp_ns().unwrap() > 0);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            counts.iter().all(|n| *n >= 16),
            "both IPv4 and IPv6 must arrive"
        );
        assert!(
            capture.rejected() > 0,
            "broad VLAN BPF branch must be checked by the exact postfilter"
        );
        let begin = Instant::now();
        capture.stop().unwrap();
        assert!(begin.elapsed() < Duration::from_millis(500));
        assert!(!capture.active());
        stop.store(true, Ordering::Release);
        sender.join().unwrap();
    });
    capture.close().unwrap();
}
