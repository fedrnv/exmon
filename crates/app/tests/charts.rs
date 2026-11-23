use exmon_app::charts::*;
fn spec(window: usize, average: usize) -> Spec {
    Spec {
        id: 1,
        name: "Chart".into(),
        samples: window,
        average,
        series: vec![Selection {
            protocol: "p".into(),
            field: "v".into(),
            label: "Voltage".into(),
            unit: "V".into(),
        }],
    }
}
fn engine(window: usize, average: usize) -> Engine {
    let mut e = Engine::default();
    assert!(e.configure(vec![spec(window, average)]));
    e.begin(1);
    e
}
#[test]
fn reference_vectors_partial_windows_gaps_and_changes() {
    for average in [1, 2, 7, 64] {
        let mut e = engine(128, average);
        let mut run = Vec::new();
        for i in 0..110 {
            let value = if i == 51 {
                None
            } else {
                Some(((i * 31) % 17) as f64 - 8.)
            };
            if let Some(v) = value {
                run.push(v);
            } else {
                run.clear();
            }
            e.sample("p", "v", i * 1000, value);
            let s = e.snapshot();
            let p = s.charts[0].series[0].points.last().unwrap();
            match value {
                Some(_) => {
                    let tail = &run[run.len().saturating_sub(average)..];
                    let reference = tail.iter().sum::<f64>() / tail.len() as f64;
                    assert!((p.average.unwrap() - reference).abs() < 1e-9);
                }
                None => assert!(p.average.is_none()),
            }
        }
    }
    let mut e = engine(16, 3);
    e.sample("p", "v", 100, Some(3.));
    e.sample("p", "v", 99, Some(5.));
    assert!(e.snapshot().charts[0].series[0].points[1].value.is_none());
    e.sample("p", "v", 101, Some(8.));
    assert_eq!(e.snapshot().charts[0].series[0].points[2].average, Some(8.));
    assert!(e.configure(vec![spec(8, 2)]));
    assert_eq!(e.snapshot().charts[0].series[0].retained, 0);
    e.begin(2);
    assert_eq!(e.snapshot().generation, 2);
}
#[test]
fn long_fixture_bounds_extrema_and_invalid_configuration() {
    let mut e = engine(4096, 17);
    for i in 0..200_000 {
        let v = if i == 199_111 {
            10000.
        } else if i == 199_777 {
            -10000.
        } else {
            (i % 101) as f64
        };
        e.sample("p", "v", i * 1_000_000, Some(v));
    }
    let view = e.snapshot();
    let s = &view.charts[0].series[0];
    assert_eq!(s.retained, 4096);
    assert_eq!(s.evicted, 200_000 - 4096);
    assert!(s.points.len() <= POINTS);
    assert_eq!(s.points.first().unwrap().index, 200_000 - 4096);
    assert_eq!(s.points.last().unwrap().index, 199_999);
    assert!(s.points.iter().any(|p| p.value == Some(10000.)));
    assert!(s.points.iter().any(|p| p.value == Some(-10000.)));
    for bad in [spec(0, 1), spec(4097, 1), spec(16, 0), spec(16, 17)] {
        assert!(!e.configure(vec![bad]));
        assert_eq!(e.snapshot().charts[0].series[0].retained, 4096);
    }
    assert!(!e.configure(vec![spec(16, 1); 5]));
    assert!(!e.configure(vec![spec(16, 1); 2]));
}
#[test]
fn gaps_survive_decimation_and_generation_never_mixes() {
    let mut e = engine(4096, 8);
    for i in 0..4096 {
        e.sample("p", "v", i, if i % 13 == 0 { None } else { Some(i as f64) });
    }
    let view = e.snapshot();
    let p = &view.charts[0].series[0].points;
    assert!(p.len() <= POINTS);
    for pair in p.windows(2) {
        if pair[0].segment == pair[1].segment {
            assert!(pair[1].index - pair[0].index < 13);
        }
    }
    e.begin(2);
    assert_eq!(e.snapshot().charts[0].series[0].retained, 0);
    e.sample("p", "v", 0, Some(f64::NAN));
    e.sample("p", "v", 1, Some(f64::INFINITY));
    assert_eq!(e.snapshot().charts[0].series[0].invalid, 2);
}
#[test]
fn independent_series_and_extreme_finite_values() {
    let mut e = engine(8, 3);
    let mut second = spec(8, 3);
    second.id = 2;
    second.series[0].field = "other".into();
    assert!(e.configure(vec![spec(8, 3), second]));
    for i in 0..8 {
        e.sample("p", "v", i, Some(f64::MAX / 2.));
        e.sample("p", "other", i, Some(i as f64));
    }
    let view = e.snapshot();
    assert!(
        view.charts[0].series[0]
            .points
            .iter()
            .all(|p| p.average.is_some_and(f64::is_finite))
    );
    assert_eq!(
        view.charts[1].series[0].points.last().unwrap().average,
        Some(6.)
    );
}
fn wait(mut f: impl FnMut() -> bool) {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(4);
    while !f() {
        assert!(std::time::Instant::now() < end, "chart worker timeout");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
#[test]
fn actual_prepared_numeric_projection_routes_to_worker_without_mutating_bytes() {
    use exmon_app::{
        incoming,
        shell::{Batch, Captured},
    };
    use exmon_domain::{
        Direction,
        observation::{Metadata, Observation},
    };
    use std::sync::Arc;
    let mut incoming = incoming::Worker::prototype().unwrap();
    wait(|| incoming.status().is_some_and(|s| s.ready));
    incoming.begin(1).unwrap();
    let charts = Worker::new().unwrap();
    charts.begin(1);
    let mut cfg = spec(16, 3);
    cfg.series[0].protocol = "sample32".into();
    cfg.series[0].field = "sequence".into();
    let revision = charts.configure(vec![cfg]).unwrap();
    wait(|| charts.snapshot().is_some_and(|s| s.revision == revision));
    let mut original = vec![];
    for (i, value) in [42u64, (1u64 << 53) + 1, 44].into_iter().enumerate() {
        let mut bytes = vec![0; 32];
        bytes[0] = 165;
        bytes[1] = 1;
        bytes[2..10].copy_from_slice(&value.to_le_bytes());
        original.push(bytes.clone());
        incoming
            .submit(Batch {
                generation: 1,
                records: vec![Captured {
                    interface_id: 1,
                    epoch: 1,
                    sequence: i as u64,
                    observation: Observation::new(
                        i as u64 * 1000,
                        Direction::Rx,
                        Metadata::Udp {
                            local: "127.0.0.1:1".parse().unwrap(),
                            remote: "127.0.0.1:2".parse().unwrap(),
                        },
                        Some(32),
                        &bytes,
                    )
                    .unwrap(),
                }],
            })
            .unwrap();
    }
    let mut rows = Vec::new();
    wait(|| {
        for b in incoming.drain() {
            rows.extend(b.rows.iter().filter(|r| !r.raw()).cloned());
            assert!(charts.submit(b));
        }
        rows.len() == 3
    });
    wait(|| {
        charts
            .snapshot()
            .is_some_and(|s| s.charts[0].series[0].retained == 3)
    });
    let s = charts.snapshot().unwrap();
    let p = &s.charts[0].series[0].points;
    assert_eq!(p[0].value, Some(42.));
    assert_eq!(p[1].value, None);
    assert_eq!(p[2].average, Some(44.));
    for (i, r) in rows.iter().enumerate() {
        assert_eq!(r.bytes(), original[i]);
        assert!(Arc::strong_count(r) >= 1);
    }
    charts.begin(2);
    wait(|| {
        charts
            .snapshot()
            .is_some_and(|s| s.generation == 2 && s.charts[0].series[0].retained == 0)
    });
    assert!(!charts.submit(incoming::PreparedBatch {
        generation: 1,
        rows: vec![rows[0].clone(); 97]
    }));
    wait(|| charts.snapshot().is_some_and(|s| s.input_dropped > 0));
}

#[test]
fn maximum_chart_series_windows_stay_bounded() {
    let mut e = Engine::default();
    let specs = (1..=4)
        .map(|id| {
            let mut s = spec(4096, 257);
            s.id = id;
            s.series = (0..8)
                .map(|i| {
                    let mut v = s.series[0].clone();
                    v.field = format!("v{i}");
                    v
                })
                .collect();
            s
        })
        .collect();
    assert!(e.configure(specs));
    for i in 0..12000 {
        for field in 0..8 {
            e.sample(
                "p",
                &format!("v{field}"),
                i * 1000,
                Some(((i * 17 + field) % 103) as f64),
            );
        }
    }
    let snapshot = e.snapshot();
    assert_eq!(snapshot.charts.len(), 4);
    for c in snapshot.charts {
        assert_eq!(c.series.len(), 8);
        for s in c.series {
            assert_eq!(s.retained, 4096);
            assert!(s.points.len() <= 512);
            assert_eq!(s.evicted, 7904);
            assert!(s.points.last().unwrap().average.is_some());
        }
    }
}
