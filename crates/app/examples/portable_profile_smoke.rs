//! Cross-platform file handoff through the production settings controller.
//! No capture or transmission worker is started.
use exmon_app::settings::{Action, Controller, Document};
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};

fn wait(mut ready: impl FnMut() -> bool) -> Result<(), io::Error> {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !ready() {
        if Instant::now() >= deadline {
            return Err(io::Error::other(
                "profile qualification controller deadline",
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}

fn perform(controller: &Controller, action: Action) -> Result<(), Box<dyn Error>> {
    let revision = controller
        .request(action)
        .map_err(|issue| io::Error::other(format!("{}: {}", issue.path, issue.message)))?;
    wait(|| {
        controller
            .snapshot()
            .is_some_and(|s| s.revision == revision)
    })?;
    let snapshot = controller
        .snapshot()
        .ok_or_else(|| io::Error::other("settings snapshot unavailable"))?;
    if let Some(issue) = &snapshot.issue {
        return Err(io::Error::other(format!("{}: {}", issue.path, issue.message)).into());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 || !matches!(args[1].as_str(), "import" | "export") {
        return Err(
            io::Error::other("usage: portable_profile_smoke import|export input output").into(),
        );
    }
    let controller = Controller::new()?;
    wait(|| {
        controller.active().is_some()
            && controller.snapshot().is_some_and(|s| !s.project.is_empty())
    })?;
    let action = if args[1] == "import" {
        Action::Import(Document::Project, args[2].clone())
    } else {
        Action::Text(Document::Project, std::fs::read_to_string(&args[2])?)
    };
    perform(&controller, action)?;
    perform(&controller, Action::Validate)?;
    perform(&controller, Action::Apply)?;
    perform(
        &controller,
        Action::Export(Document::Project, args[3].clone()),
    )?;
    let bytes = std::fs::read(&args[3])?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(value["protocols"][0]["fields"][0]["endian"], "little");
    assert_eq!(value["protocols"][0]["fields"][0]["unit"], "V");
    assert_eq!(value["protocols"][0]["fields"][0]["scale"], 0.5);
    assert_eq!(value["charts"][0]["moving_average"], 4);
    assert_eq!(value["schedules"][0]["enabled"], false);
    assert!(!std::str::from_utf8(&bytes)?.contains("selector"));
    println!("PASS: portable profile validated/applied/exported without starting I/O");
    Ok(())
}
