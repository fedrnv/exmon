use exmon_app::{
    incoming::*,
    shell::{Batch, CaptureWorker, Captured},
};
use exmon_domain::{
    Direction,
    observation::{Metadata, Observation, TransportKind},
};
use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
fn wait(mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(3);
    while !f() {
        assert!(Instant::now() < end, "incoming fixture timeout");
        thread::sleep(Duration::from_millis(1));
    }
}
fn worker() -> Worker {
    let w = Worker::prototype().unwrap();
    wait(|| w.status().is_some_and(|s| s.ready));
    w.begin(1).unwrap();
    wait(|| w.status().is_some_and(|s| s.generation == 1));
    w
}
fn sample(sequence: u64) -> Vec<u8> {
    let mut b = vec![0; 32];
    b[0] = 165;
    b[1] = 1;
    b[2..10].copy_from_slice(&sequence.to_le_bytes());
    b
}
fn record(sequence: u64, bytes: &[u8], stream: bool, known: bool) -> Captured {
    Captured {
        interface_id: 1,
        epoch: 1,
        sequence,
        observation: Observation::new(
            sequence * 1000,
            Direction::Rx,
            if stream {
                Metadata::Serial {
                    configuration_revision: 1,
                }
            } else {
                Metadata::Udp {
                    local: "127.0.0.1:1".parse().unwrap(),
                    remote: "127.0.0.1:2".parse().unwrap(),
                }
            },
            known.then_some(bytes.len()),
            bytes,
        )
        .unwrap(),
    }
}
fn send(w: &Worker, records: Vec<Captured>) {
    w.submit(Batch {
        generation: 1,
        records,
    })
    .unwrap();
}
fn rows(w: &mut Worker) -> Vec<Arc<Row>> {
    let mut out = vec![];
    wait(|| {
        for b in w.drain() {
            out.extend(b.rows);
        }
        !out.is_empty()
    });
    out
}
#[test]
fn actual_worker_classifies_decodes_counts_rates_and_copies_without_authority() {
    let mut w = worker();
    assert_ne!(w.worker_thread().unwrap(), thread::current().id());
    assert_eq!(w.status().unwrap().processed, 0);
    let bytes = sample(1234);
    send(&w, vec![record(1, &bytes, false, true)]);
    let r = rows(&mut w);
    assert_eq!(r.len(), 2);
    assert!(r[0].raw());
    assert_eq!(r[1].group.as_deref(), Some("rule:sample32"));
    assert_eq!(r[1].fields[0].value.trim(), "1234");
    assert_eq!(r[1].source, "127.0.0.1:2");
    assert_eq!(r[1].destination, "127.0.0.1:1");
    let draft = r[1].copy_format().unwrap();
    assert_eq!(draft.row.bytes(), bytes);
    assert!(r[0].copy_format().is_none());
    assert_eq!(draft.row.sequence, 1);
    assert_eq!(draft.row.protocol.as_ref().unwrap().fields.len(), 2);
    wait(|| w.status().is_some_and(|s| s.groups.len() == 1));
    let g = w.status().unwrap().groups.remove(0);
    assert_eq!(g.count, 1);
    assert_eq!(g.bytes, 32);
    assert!(g.per_second.unwrap().is_finite());
    assert!(g.per_second.unwrap() > 0.);
}
#[test]
fn stream_fragments_keep_raw_provenance_and_unknown_quality_without_false_completion() {
    let mut w = worker();
    let bytes = sample(99);
    send(&w, vec![record(1, &bytes[..16], true, false)]);
    let r = rows(&mut w);
    assert_eq!(r.len(), 1);
    assert!(r[0].status.contains("fragment"));
    send(&w, vec![record(2, &bytes[16..], true, true)]);
    let r = rows(&mut w);
    let message = r.iter().find(|r| !r.raw()).unwrap();
    assert_eq!(message.bytes(), bytes);
    assert_eq!(
        message.quality,
        Some(exmon_protocol::classification::Quality::Unknown)
    );
    assert_eq!(message.sources.len(), 2);
    assert_eq!(message.sources[0].observation, 1);
    assert_eq!(message.sources[1].observation, 2);
    send(&w, vec![record(3, &bytes[..16], true, true)]);
    assert_eq!(rows(&mut w).len(), 1);
    send(&w, vec![record(5, &bytes[16..], true, true)]);
    let r = rows(&mut w);
    assert_eq!(r.len(), 1);
    assert!(r[0].status.contains("gap"));
    let mut w = worker();
    send(&w, vec![record(0, &bytes[..16], true, true)]);
    assert_eq!(rows(&mut w).len(), 1);
    send(&w, vec![record(2, &bytes[16..], true, true)]);
    let r = rows(&mut w);
    assert_eq!(r.len(), 1);
    assert!(r[0].status.contains("gap"));
}
#[test]
fn names_are_explicit_validated_config_edits_old_instances_are_immutable() {
    let mut w = worker();
    send(&w, vec![record(1, &sample(1), false, true)]);
    let old = rows(&mut w).pop().unwrap();
    assert!(w.rename("rule:sample32", "").is_err());
    w.rename("rule:missing", "New").unwrap();
    wait(|| w.status().is_some_and(|s| s.rename_error));
    w.rename("rule:sample32", "Telemetry renamed").unwrap();
    wait(|| {
        w.configuration()
            .is_some_and(|c| c.protocols[0].name == "Telemetry renamed")
    });
    send(&w, vec![record(2, &sample(2), false, true)]);
    let new = rows(&mut w).pop().unwrap();
    assert_eq!(new.name, "Telemetry renamed");
    assert_eq!(old.name, "Sample telemetry");
    assert!(new.revision > old.revision);
    assert!(w.configuration().unwrap().schedules.is_empty());
    send(&w, vec![record(3, &[1, 2, 3], false, true)]);
    let auto = rows(&mut w).pop().unwrap();
    let key = auto.group.as_ref().unwrap();
    assert!(key.starts_with("auto:"));
    w.rename(key, "Unrecognized status").unwrap();
    wait(|| {
        w.configuration().is_some_and(|c| {
            c.grouping
                .names
                .first()
                .is_some_and(|n| n.name == "Unrecognized status")
        })
    });
    send(&w, vec![record(4, &[1, 2, 3], false, true)]);
    let renamed = rows(&mut w).pop().unwrap();
    assert_eq!(renamed.group, auto.group);
    assert_eq!(renamed.name, "Unrecognized status");
}
#[test]
fn selected_context_survives_updates_eviction_pause_and_generation_reset() {
    let mut w = worker();
    send(&w, vec![record(1, &sample(1), false, true)]);
    let r = rows(&mut w);
    let selected = r[1].clone();
    let mut view = View::default();
    view.reset(1);
    view.apply(PreparedBatch {
        generation: 1,
        rows: r,
    });
    view.select_tab(Some("rule:sample32".into()));
    assert_eq!(view.selected.as_ref().unwrap().key, selected.key);
    for chunk in 0..36 {
        send(
            &w,
            (0..32)
                .map(|n| record(100 + chunk * 32 + n, &sample(n), false, true))
                .collect(),
        );
        view.apply(PreparedBatch {
            generation: 1,
            rows: rows(&mut w),
        });
    }
    assert!(view.rows.len() <= RETAINED);
    assert!(view.retained_bytes() <= RETAINED_BYTES);
    assert!(view.evicted > 0);
    assert!(view.selected_evicted());
    assert_eq!(view.selected.as_ref().unwrap().bytes(), sample(1));
    send(&w, vec![record(2, &sample(2), false, true)]);
    let next = rows(&mut w);
    view.paused = true;
    let retained = view.rows.len();
    view.apply(PreparedBatch {
        generation: 1,
        rows: next,
    });
    assert_eq!(view.rows.len(), retained);
    assert_eq!(view.paused_skipped, 2);
    assert_eq!(view.selected.as_ref().unwrap().bytes(), sample(1));
    view.reset(2);
    view.apply(PreparedBatch {
        generation: 1,
        rows: vec![selected],
    });
    assert!(view.rows.is_empty());
    assert!(view.selected.is_none());
    w.begin(2).unwrap();
    assert_eq!(w.begin(1), Err(Error::Invalid));
}
#[test]
fn automatic_group_capacity_and_slow_consumer_are_explicit_and_bounded() {
    let mut w = worker();
    for chunk in 0..5 {
        send(
            &w,
            (0..32)
                .map(|n| {
                    record(
                        chunk * 32 + n + 1,
                        &[0, (chunk * 32 + n) as u8, 0, 0],
                        false,
                        true,
                    )
                })
                .collect(),
        );
    }
    wait(|| w.status().is_some_and(|s| s.processed == 160));
    let s = w.status().unwrap();
    assert_eq!(s.groups.len(), 64);
    assert_eq!(s.group_overflow, 96);
    let mut view = View::default();
    view.reset(1);
    for batch in w.drain() {
        view.apply(batch);
    }
    assert!(view.rows.len() <= 1024);
    let mut failures = 0;
    for _ in 0..100 {
        if w.submit(Batch {
            generation: 1,
            records: (0..32)
                .map(|n| record(1000 + n, &sample(n), false, true))
                .collect(),
        })
        .is_err()
        {
            failures += 1;
        }
    }
    assert!(failures > 0);
    assert!(w.status().unwrap().input_dropped > 0);
    wait(|| w.status().is_some_and(|s| s.output_decimation > 0));
}
#[test]
fn invalid_configuration_fails_closed_and_missing_framing_preserves_raw() {
    let config = exmon_domain::config::ProjectConfig {
        schema_version: 999,
        ..Default::default()
    };
    let w = Worker::configured(config, None).unwrap();
    wait(|| w.status().is_some_and(|s| s.failed));
    assert!(!w.status().unwrap().ready);
    let mut w = Worker::configured(exmon_domain::config::ProjectConfig::default(), None).unwrap();
    wait(|| w.status().is_some_and(|s| s.ready));
    w.begin(1).unwrap();
    send(&w, vec![record(1, &sample(1), true, true)]);
    let r = rows(&mut w);
    assert_eq!(r.len(), 1);
    assert!(r[0].status.contains("not configured"));
}
#[test]
fn paused_processing_consumer_never_stops_actual_capture() {
    let mut capture = CaptureWorker::spawn().unwrap();
    let w = worker();
    capture.start(TransportKind::Udp, 1).unwrap();
    wait(|| capture.status().packets > 512);
    let before = capture.status().packets;
    for _ in 0..12 {
        for batch in capture.drain(4) {
            let _ = w.submit(batch);
        }
        thread::sleep(Duration::from_millis(3));
    }
    assert!(capture.status().packets > before);
    capture.stop();
    wait(|| !capture.status().running);
}
#[test]
fn incomplete_and_invalid_field_values_remain_visible_without_fabricated_zero() {
    let mut w = worker();
    let bytes = sample(1234);
    let mut partial = record(1, &bytes, false, true);
    partial.observation = Observation::new(
        1000,
        Direction::Rx,
        partial.observation.metadata().clone(),
        Some(64),
        &bytes,
    )
    .unwrap();
    send(&w, vec![partial]);
    let r = rows(&mut w);
    assert_eq!(r[1].status, "Incomplete");
    assert!(r[1].fields.is_empty());
    assert_eq!(r[1].bytes(), bytes);
    let mut config = (*w.configuration().unwrap()).clone();
    config.protocols[0].fields[0].constant =
        Some(exmon_domain::fields::Literal::Unsigned { value: 1 });
    let mut invalid = Worker::configured(config, None).unwrap();
    wait(|| invalid.status().is_some_and(|s| s.ready));
    invalid.begin(1).unwrap();
    send(&invalid, vec![record(1, &bytes, false, true)]);
    let r = rows(&mut invalid);
    assert!(r[1].fields[0].invalid);
    assert_eq!(r[1].fields[0].value.trim(), "1234");
}
#[test]
fn large_native_records_enforce_byte_retention_and_input_admission_limits() {
    let mut w = worker();
    let bytes = vec![1; 65536];
    let mut view = View::default();
    view.reset(1);
    for n in 0..40 {
        send(&w, vec![record(n, &bytes, false, true)]);
        view.apply(PreparedBatch {
            generation: 1,
            rows: rows(&mut w),
        });
    }
    assert!(view.retained_bytes() <= RETAINED_BYTES);
    assert!(view.rows.len() < RETAINED);
    assert!(view.evicted > 0);
    assert_eq!(
        w.submit(Batch {
            generation: 1,
            records: vec![
                record(50, &bytes, false, true),
                record(51, &bytes, false, true)
            ]
        }),
        Err(Error::Invalid)
    );
    assert_eq!(
        w.submit(Batch {
            generation: 1,
            records: (0..33).map(|n| record(n, &[0], false, true)).collect()
        }),
        Err(Error::Invalid)
    );
}
