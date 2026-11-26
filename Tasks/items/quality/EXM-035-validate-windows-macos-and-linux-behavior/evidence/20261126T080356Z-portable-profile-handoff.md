# EXM-035 Windows-to-Linux portable profile handoff

Source: 58f5eb938998e6ee9d5dffd4f5f430be2c7da3b6; product correction EXM-101 at 9b53499.
Rust 1.98.0, committed workspace dependency versions, Windows x64 host and
actual Ubuntu 24.04.5 LTS x64 guest under QEMU 11.1.0 q35 TCG (4 vCPUs,
4 GiB RAM). The Ubuntu cloud image was downloaded from the official noble
current endpoint and matched the published SHA-256:
04e8f2461a16a44e78a370284471f29616cdd9baf1893df25807b71f985b6c94.
The writable overlay is synthetic, ignored and has no physical passthrough.
Guest public-package networking and SSH are separate from product interfaces;
SSH is forwarded only on host loopback to an ephemeral unprivileged guest user.
No existing host credentials or devices were supplied to the guest.

A bounded standalone qualification harness uses the public settings Controller:
Text or Import, Validate, Apply, Export. It starts no capture or transmission
worker. The synthetic profile includes a fixed-length four-byte protocol,
little-endian numeric field with scale 0.5/bias 1/unit V, a 32-sample chart with
four-sample moving average, one four-byte TX template and a disabled finite
100 ms schedule. A TX-permitted profile flag does not authorize physical I/O.
Machine bindings are absent. Assertions check scale, unit, endianness, moving
average, disabled schedule and absence of machine selectors.

Windows exported the file through the product controller. Those exact bytes
were copied over the local guest SSH connection. The actual Linux GNU harness
imported the same file and exported it again. Guest cmp succeeded and both
files had SHA-256 8f8b9c51a2a0ae2ca05c9d350ef24ba6fb126beba2e00c980ac1537edc6950db.
The runner emitted PORTABLE_BYTES_IDENTICAL and exited 0.
This establishes the tested Windows/Linux file handoff; Apple Silicon remains
unverified and no packaged install or physical-device verdict follows.

Windows built the harness with rustc against the current app/serde_json rlibs.
Direct cross-target rustc linking could not resolve exmon_app metadata, so the
Linux harness was built with an ignored standalone Cargo manifest pointing to
the current app source and serde_json 1.0.154, a copy of the workspace lockfile,
--offline, --target x86_64-unknown-linux-gnu, and the verified Zig 0.15.2 cc
wrapper targeting x86_64-linux-gnu.2.35. The workspace lockfile was unchanged.
Rust llvm-objcopy --strip-debug only removed debug sections before transfer.
All controller operations have a 30-second harness deadline. This harness
deadline does not modify or replace any existing product/test deadline.

## Reproducible harness source

```rust
use exmon_app::settings::{Action, Controller, Document};
use std::time::{Duration, Instant};
fn wait(mut f: impl FnMut() -> bool) { let end=Instant::now()+Duration::from_secs(30);while !f(){assert!(Instant::now()<end,"qualification controller deadline");std::thread::sleep(Duration::from_millis(5));}}
fn perform(c:&Controller,a:Action){let r=c.request(a).expect("accepted operation");wait(||c.snapshot().is_some_and(|s|s.revision==r));let s=c.snapshot().unwrap();assert!(s.issue.is_none(),"settings issue: {:?}",s.issue.as_ref().map(|i|(&i.path,&i.message)));}
fn main(){
 let args:Vec<_>=std::env::args().collect();assert_eq!(args.len(),4,"mode input output");
 let c=Controller::new().unwrap();wait(||c.active().is_some()&&c.snapshot().is_some_and(|s|!s.project.is_empty()));
 if args[1]=="export"{perform(&c,Action::Text(Document::Project,std::fs::read_to_string(&args[2]).unwrap()));}else{assert_eq!(args[1],"import");perform(&c,Action::Import(Document::Project,args[2].clone()));}
 perform(&c,Action::Validate);perform(&c,Action::Apply);perform(&c,Action::Export(Document::Project,args[3].clone()));
 let exported:serde_json::Value=serde_json::from_slice(&std::fs::read(&args[3]).unwrap()).unwrap();
 assert_eq!(exported["protocols"][0]["fields"][0]["endian"],"little");assert_eq!(exported["protocols"][0]["fields"][0]["unit"],"V");assert_eq!(exported["protocols"][0]["fields"][0]["scale"],0.5);assert_eq!(exported["schedules"][0]["enabled"],false);assert_eq!(exported["charts"][0]["moving_average"],4);assert!(!std::fs::read_to_string(&args[3]).unwrap().contains("selector"));
 println!("PASS portable profile via public settings controller: {} -> {}",args[2],args[3]);
}
```

## Synthetic profile input

```json
{"schema_version":1,"protocols":[{"id":"p","name":"Synthetic voltage","framing":{"kind":"fixed","length":4},"classification":{"length":4,"prefix":[165]},"fields":[{"id":"word","offset":1,"length":2,"representation":"unsigned","endian":"little","scale":0.5,"bias":1.0,"unit":"V"}]}],"interfaces":[{"id":"port","protocol_id":"p","transport":{"kind":"udp"},"filter":{"direction":"Rx","ports":[42]},"tx_allowed":true}],"charts":[{"id":"voltage","protocol_id":"p","field_id":"word","samples":32,"moving_average":4}],"templates":[{"id":"frame","interface_id":"port","bytes":[165,1,2,0]}],"schedules":[{"id":"periodic","template_id":"frame","interval_ms":100,"count":3,"enabled":false}]}
```

## Local evidence hashes

- `portable.rs`: `792eb5801d757111ada8170fba6c3ccfaf73005be9ab4125a532a135e9e8b878`
- `portable-input.json`: `7ee32cfbb5932e1bacfd673c91d4be97df9d8ed2c2976fa71ebdbe220a21c54f`
- `portable.exe`: `266a6bafde99de8eb6b60549c7862a7a86448ce2679de8c741e7cf48fa07057e`
- `portable-linux-gnu`: `61fcd7786cf31e05696170a3bfb0f583f5ab840e8a9b9f8f03411c146b59aaff`
- `portable-windows.json`: `8f8b9c51a2a0ae2ca05c9d350ef24ba6fb126beba2e00c980ac1537edc6950db`
- `portable-runtime.log`: `0c19ec0d1e2b3ee0871f01f62982aaf25fc12eb7a827d87d6305889341df0b76`
- `portable-build.log`: `486c4d8bb04efe0bdadb02f3589fb249dfda637b65302aa6673882c206c3e75c`
- `guest-portable.sh`: `2841e339b15386a5b262de04b4e99eb9583e4a8baacff393e470124c19e7d33f`

Bounded synthetic files/tooling stay ignored under target/exm035-linux. No
SSH private key, guest image, executable or build output is committed.
