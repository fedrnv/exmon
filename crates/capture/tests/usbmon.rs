use exmon_capture::{
    adapter::{Availability, Error},
    native_usbmon,
    usbmon::*,
};
use exmon_domain::{Direction, observation::Metadata};
use std::io::{self, Cursor, Read};
#[test]
fn bulk_interrupt_control_and_iso_preserve_metadata_and_bytes() {
    let record = parse("1 123 C Bi:2:003:1 0 3 = a5002a").unwrap();
    assert_eq!(record.bytes, [165, 0, 42]);
    assert_eq!(record.metadata.bus, 2);
    assert_eq!(record.metadata.device, 3);
    let observation = record.into_observation(7).unwrap();
    assert_eq!(observation.timestamp_ns(), 7);
    assert_eq!(observation.source_timestamp_ns(), Some(123000));
    assert_eq!(observation.direction(), Direction::Rx);
    assert!(matches!(observation.metadata(), Metadata::UsbMon(_)));
    let interrupt = parse("2 124 S Io:2:003:2 -115:8 4 = 01020304").unwrap();
    assert_eq!(interrupt.metadata.interval, Some(8));
    assert_eq!(interrupt.metadata.status, Some(-115));
    assert_eq!(
        interrupt.into_observation(8).unwrap().direction(),
        Direction::Tx
    );
    let setup = parse("3 125 S Ci:2:003:0 s 80 06 0100 0000 0012 18 <").unwrap();
    assert_eq!(setup.metadata.setup, Some([0x80, 6, 0, 1, 0, 0, 18, 0]));
    assert_eq!(setup.metadata.setup_tag, Some(b's'));
    assert!(setup.bytes.is_empty());
    assert_eq!(setup.metadata.reported_length, 18);
    let iso = parse("4 126 C Zi:2:003:3 0:1:5:0 1 0:0:2 2 = 01020000").unwrap();
    assert_eq!(iso.metadata.iso_count, 1);
    assert_eq!(iso.metadata.iso[0], Some((0, 0, 2)));
    assert_eq!(iso.metadata.start_frame, Some(5));
    assert_eq!(iso.bytes, [1, 2, 0, 0]);
    assert_eq!(iso.into_observation(9).unwrap().original_length(), None);
    let failure = parse("5 127 E Bo:2:003:1 -32 0 >").unwrap();
    assert_eq!(failure.metadata.event, b'E');
    assert_eq!(failure.metadata.status, Some(-32));
}
#[test]
fn incomplete_data_and_invalid_records_are_explicit() {
    let short = parse("1 1 C Bi:1:001:1 0 8 = 0102")
        .unwrap()
        .into_observation(0)
        .unwrap();
    assert_eq!(short.original_length(), Some(8));
    assert_eq!(short.bytes(), [1, 2]);
    for line in [
        "",
        "1 0 C Bi:1:1 0 1 = 01",
        "1 0 X Bi:1:1:1 0 1 = 01",
        "1 0 C Bi:1:128:1 0 1 = 01",
        "1 0 C Bi:1:1:16 0 1 = 01",
        "1 0 C Bi:1:1:1 0 1 = 0102",
        "1 0 C Bi:1:1:1 0 1 = gg",
        "1 0 C Bi:1:1:1 0 1 = 1",
        "1 18446744073709551615 C Bi:1:1:1 0 0 >",
        "1 0 C Bi:1:1:1 0 1 > 01",
    ] {
        assert!(parse(line).is_err(), "{line}");
    }
    assert!(parse(&" ".repeat(MAX_LINE + 1)).is_err());
}
struct Split {
    parts: Vec<&'static [u8]>,
    index: usize,
    blocked: bool,
}
impl Read for Split {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.blocked {
            self.blocked = false;
            return Err(io::ErrorKind::WouldBlock.into());
        }
        if self.index == self.parts.len() {
            return Ok(0);
        }
        let part = self.parts[self.index];
        assert!(buffer.len() >= part.len());
        buffer[..part.len()].copy_from_slice(part);
        self.index += 1;
        self.blocked = true;
        Ok(part.len())
    }
}
#[test]
fn incremental_nonblocking_reader_preserves_lines_and_reports_truncated_eof() {
    let mut reader = Reader::new(Split {
        parts: vec![b"1 0 C Bi:1:001:1 0 ", b"2 = a5", b"00\n"],
        index: 0,
        blocked: false,
    });
    assert!(reader.next_record().unwrap().is_none());
    assert!(reader.next_record().unwrap().is_none());
    assert_eq!(reader.next_record().unwrap().unwrap().bytes, [165, 0]);
    let mut reader = Reader::new(Cursor::new(b"1 0 C Bi:1:1:1 0 2 = 0102"));
    assert_eq!(reader.next_record().err(), Some(Error::Backend));
    assert!(reader.ended());
    let mut reader = Reader::new(Cursor::new(
        b"1 0 C Bi:1:1:1 0 1 = 01\n2 1 C Bi:1:1:1 0 1 = 02\n",
    ));
    assert_eq!(reader.next_record().unwrap().unwrap().bytes, [1]);
    assert_eq!(reader.next_record().unwrap().unwrap().bytes, [2]);
}
#[test]
fn unsupported_hosts_cannot_fall_back_to_active_usb() {
    if !cfg!(all(target_os = "linux", feature = "native-usbmon")) {
        assert_eq!(native_usbmon::capability(), Availability::Unsupported);
        let mut native = native_usbmon::NativeUsbmon::default();
        assert_eq!(native.enumerate().err(), Some(Error::Unsupported));
        assert_eq!(native.open(0), Err(Error::Unsupported));
    }
    assert!(!native_usbmon::requirements().is_empty());
}

#[test]
fn oversized_line_is_rejected_and_following_record_recovers() {
    let mut input = vec![b'x'; MAX_LINE + 99];
    input.extend_from_slice(b"\n1 0 C Bi:1:1:1 0 1 = a5\n");
    let mut reader = Reader::new(Cursor::new(input));
    let mut rejected = 0;
    let mut bytes = None;
    for _ in 0..100 {
        match reader.next_record() {
            Err(Error::Backend) => rejected += 1,
            Ok(Some(record)) => {
                bytes = Some(record.bytes);
                break;
            }
            Ok(None) => {}
            Err(other) => panic!("unexpected {other}"),
        }
    }
    assert_eq!(rejected, 1);
    assert_eq!(bytes, Some(vec![165]));
}

#[test]
fn common_session_preserves_passive_direction_limits_and_cancellation() {
    use exmon_capture::adapter::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Fixture {
        reads: Arc<AtomicUsize>,
        active: bool,
    }
    impl Source for Fixture {
        fn buses(&mut self) -> Result<Vec<u16>, Error> {
            Ok(vec![2])
        }
        fn activate(&mut self, bus: u16) -> Result<(), Error> {
            assert_eq!(bus, 2);
            self.active = true;
            Ok(())
        }
        fn next(&mut self) -> Result<Option<Record>, Error> {
            assert!(self.active);
            self.reads.fetch_add(1, Ordering::SeqCst);
            parse("1 12 S Bo:2:3:1 -115 4 = 01020304").map(Some)
        }
        fn close(&mut self) {
            self.active = false;
        }
    }
    let reads = Arc::new(AtomicUsize::new(0));
    let mut adapter = Usbmon::new(Fixture {
        reads: reads.clone(),
        active: false,
    });
    assert_eq!(
        adapter.enumerate(1).unwrap()[0].tx,
        Availability::Unsupported
    );
    assert_eq!(adapter.transmit(&[1]), Err(Error::Unsupported));
    let mut session = Session::new(
        adapter,
        Limits {
            queue_items: 2,
            queue_bytes: 4,
            batch_items: 2,
            batch_bytes: 4,
        },
    )
    .unwrap();
    session.open(2).unwrap();
    session.configure(2).unwrap();
    assert_eq!(reads.load(Ordering::SeqCst), 0);
    session.start().unwrap();
    session.pump().unwrap();
    let records = session.drain(2);
    assert_eq!(records.len(), 2);
    for record in records {
        assert_eq!(record.observation.bytes(), [1, 2]);
        assert_eq!(record.observation.original_length(), Some(4));
        assert_eq!(record.observation.source_timestamp_ns(), Some(12000));
        assert_eq!(record.observation.direction(), Direction::Tx);
    }
    session.cancellation().cancel();
    assert_eq!(session.pump(), Err(Error::Cancelled));
    assert_eq!(reads.load(Ordering::SeqCst), 2);
    assert_eq!(session.state(), State::Stopped);
    session.close().unwrap();
}
