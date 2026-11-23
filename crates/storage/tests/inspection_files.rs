use exmon_storage::inspection_files::{MAX_BYTES, open};
use std::{
    fs::{self, File},
    io::Read,
};
#[test]
fn bounded_regular_local_files_and_unsupported_paths() {
    let root = std::env::temp_dir().join(format!("exmon030-storage-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let p = root.join("input.pcap");
    fs::write(&p, b"capture").unwrap();
    let mut bytes = Vec::new();
    open(&p).unwrap().read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"capture");
    assert!(open(&root).is_err());
    let f = File::create(&p).unwrap();
    f.set_len(MAX_BYTES + 1).unwrap();
    drop(f);
    assert!(open(&p).is_err());
    assert!(open(std::path::Path::new("bad\npath")).is_err());
    #[cfg(windows)]
    for path in [
        r"\\server\share\capture",
        r"\\.\PhysicalDrive0",
        "NUL",
        "capture:stream",
    ] {
        assert!(open(std::path::Path::new(path)).is_err());
    }
    fs::remove_file(p).unwrap();
    fs::remove_dir(root).unwrap();
}
