use crate::egui;
use exmon_app::{
    network_filters::{FilterPlan, NetworkFilter},
    shell::CaptureStatus,
};
use exmon_domain::{Direction, observation::TransportKind};

#[derive(Default)]
pub(crate) struct FilterPanel {
    ports: String,
    source: String,
    destination: String,
    host: String,
    protocol: Option<u8>,
    direction: Option<Direction>,
}
pub(crate) fn network(kind: TransportKind) -> bool {
    matches!(
        kind,
        TransportKind::Ethernet | TransportKind::Tcp | TransportKind::Udp
    )
}
impl FilterPanel {
    pub(crate) fn settings(&self, interface: u64) -> Result<NetworkFilter, &'static str> {
        let address = |text: &str| {
            if text.trim().is_empty() {
                Ok(None)
            } else if text.len() > 64 {
                Err("IP address is too long.")
            } else {
                text.trim()
                    .parse()
                    .map(Some)
                    .map_err(|_| "Enter a numeric IPv4 or IPv6 address.")
            }
        };
        if self.ports.len() > 192 {
            return Err("At most 32 comma-separated ports are allowed.");
        }
        let mut ports = Vec::new();
        if !self.ports.trim().is_empty() {
            for port in self.ports.split(',') {
                if ports.len() == 32 {
                    return Err("At most 32 ports are allowed.");
                }
                ports.push(
                    port.trim()
                        .parse::<u16>()
                        .map_err(|_| "Ports must be comma-separated numbers from 0 to 65535.")?,
                );
            }
        }
        let settings = NetworkFilter {
            interface: Some(interface),
            direction: self.direction,
            protocol: self.protocol,
            source: address(&self.source)?,
            destination: address(&self.destination)?,
            host: address(&self.host)?,
            ports,
        };
        settings.validate()?;
        Ok(settings)
    }
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        interface: u64,
        kind: TransportKind,
        status: CaptureStatus,
        effective: Option<FilterPlan>,
    ) {
        if !network(kind) {
            ui.label("Network filters are available for Ethernet, TCP and UDP.");
            return;
        }
        ui.add_enabled_ui(!status.requested && !status.running, |ui| {
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_label("IP protocol")
                    .selected_text(match self.protocol {
                        None => "Any",
                        Some(6) => "TCP",
                        _ => "UDP",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.protocol, None, "Any");
                        ui.selectable_value(&mut self.protocol, Some(6), "TCP");
                        ui.selectable_value(&mut self.protocol, Some(17), "UDP");
                    });
                egui::ComboBox::from_label("Direction")
                    .selected_text(match self.direction {
                        None => "Any",
                        Some(Direction::Rx) => "RX",
                        Some(Direction::Tx) => "TX",
                        Some(Direction::Unknown) => "Unknown",
                    })
                    .show_ui(ui, |ui| {
                        for (value, label) in [
                            (None, "Any"),
                            (Some(Direction::Rx), "RX"),
                            (Some(Direction::Tx), "TX"),
                            (Some(Direction::Unknown), "Unknown"),
                        ] {
                            ui.selectable_value(&mut self.direction, value, label);
                        }
                    });
                let label = ui.label("Either port");
                ui.add(
                    egui::TextEdit::singleline(&mut self.ports)
                        .id_salt("filter-ports")
                        .hint_text("80,443")
                        .desired_width(150.)
                        .char_limit(192),
                )
                .labelled_by(label.id);
            });
            ui.horizontal_wrapped(|ui| {
                for (label, text) in [
                    ("Source IP", &mut self.source),
                    ("Destination IP", &mut self.destination),
                    ("Either IP", &mut self.host),
                ] {
                    let label = ui.label(label);
                    ui.add(
                        egui::TextEdit::singleline(text)
                            .id_salt(label.id)
                            .hint_text("Any")
                            .desired_width(145.)
                            .char_limit(64),
                    )
                    .labelled_by(label.id);
                }
            });
        });
        if status.requested || status.running {
            if let Some(plan) = effective {
                ui.label(format!(
                    "{} filter: {}",
                    if status.running {
                        "Running"
                    } else {
                        "Starting"
                    },
                    plan.summary()
                ));
            }
        } else {
            match self.settings(interface).and_then(FilterPlan::new) {
                Ok(plan) => {
                    ui.label(format!("Ready filter: {}", plan.summary()));
                }
                Err(error) => {
                    ui.colored_label(egui::Color32::from_rgb(230, 140, 90), error);
                }
            }
        }
        ui.label(egui::RichText::new("Mock: exact postfilter only | Ethernet mock data has no IP headers | No physical capture or TX").small().weak());
    }
}
