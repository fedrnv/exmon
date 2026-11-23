use exmon_storage::{config::MAX_DOCUMENT_BYTES, settings_files::*};
#[test]
fn bounded_regular_file_create_read_and_no_overwrite() {
    let path = std::env::temp_dir().join(format!(
        "exmon-settings-file-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    assert_eq!(create(&path, b"{}"), Ok(()));
    assert_eq!(read(&path), Ok(b"{}".to_vec()));
    assert_eq!(create(&path, b"overwrite"), Err(Error::Exists));
    assert_eq!(std::fs::read(&path).unwrap(), b"{}");
    std::fs::write(&path, vec![0; MAX_DOCUMENT_BYTES + 1]).unwrap();
    assert_eq!(read(&path), Err(Error::TooLarge));
    assert_eq!(
        create(&path, &vec![0; MAX_DOCUMENT_BYTES + 1]),
        Err(Error::TooLarge)
    );
    std::fs::remove_file(path).unwrap();
}
#[cfg(windows)]
#[test]
fn windows_device_network_and_alternate_stream_paths_are_rejected() {
    for p in [
        r"\\.\pipe\exmon-test",
        r"\\server\share\profile.json",
        r"C:\tmp\NUL",
        r"C:\tmp\file.json:secret",
        r"C:\tmp\COM1",
    ] {
        assert_eq!(read(std::path::Path::new(p)), Err(Error::Read));
        assert_eq!(create(std::path::Path::new(p), b"{}"), Err(Error::Read));
    }
}
