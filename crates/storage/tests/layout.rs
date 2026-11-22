use exmon_storage::layout::*;
#[test]
fn layout_is_bounded_versioned_and_cannot_restore_capture_or_transmission() {
    let safe = Layout::default();
    assert!(safe.valid());
    for bytes in [
        b"{}".as_slice(),
        br#"{"schema":2,"width":1200,"height":760,"split":0.5,"theme":"Dark"}"#,
        br#"{"schema":1,"width":1200,"height":760,"split":0.5,"theme":"Dark","running":true}"#,
        br#"{"schema":1,"width":1,"height":760,"split":0.5,"theme":"Dark"}"#,
        br#"{"schema":1,"width":1200,"height":760,"split":9,"theme":"Dark"}"#,
    ] {
        assert_eq!(Layout::decode(bytes), Err(LayoutError::Invalid));
    }
    assert_eq!(Layout::decode(&vec![b' '; 4097]), Err(LayoutError::Invalid));
    let path = std::env::temp_dir().join(format!(
        "exmon-layout-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    assert_eq!(load(&path), Ok(None));
    save(&path, safe).unwrap();
    let next = Layout {
        theme: Theme::Light,
        width: 1360.,
        split: 0.7,
        ..safe
    };
    save(&path, next).unwrap();
    assert_eq!(load(&path), Ok(Some(next)));
    let mut invalid = next;
    invalid.width = f32::NAN;
    assert_eq!(save(&path, invalid), Err(LayoutError::Invalid));
    assert_eq!(load(&path), Ok(Some(next)));
    std::fs::remove_file(path).unwrap();
}
