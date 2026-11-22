//! Manual operator-only smoke tool; no TX and no default physical capture.
use exmon_capture::{
    adapter::{Adapter, Cancellation},
    ethernet::{Ethernet, Options},
    native_pcap::NativePcap,
};
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args != ["--list"] && !(args.len() == 2 && args[0] == "--capture") {
        return Err(
            "use --list, or explicitly --capture <interface-id> on an authorized test interface"
                .into(),
        );
    }
    let mut adapter = Ethernet::new(NativePcap::default(), Options::default())?;
    let descriptors = adapter.enumerate(64)?;
    if args == ["--list"] {
        for d in descriptors {
            println!(
                "{} {}",
                d.id,
                adapter.interface_name(d.id).unwrap_or("unavailable")
            );
        }
        return Ok(());
    }
    let id = args[1].parse()?;
    adapter.open(id)?;
    adapter.configure(2048)?;
    adapter.start()?;
    let started = Instant::now();
    let token = Cancellation::default();
    let mut records = 0;
    while started.elapsed() < Duration::from_secs(1) {
        records += adapter.poll(&token, 16, 32768)?.len();
        std::thread::sleep(Duration::from_millis(1));
    }
    token.cancel();
    adapter.stop()?;
    adapter.close()?;
    println!("capture stopped; observed {records} records; no payload retained");
    Ok(())
}
