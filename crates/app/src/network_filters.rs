//! Applies the same immutable plan before and after bounded Ethernet capture.
use exmon_capture::{
    adapter::{Adapter, Cancellation, Descriptor, Error},
    ethernet::{Ethernet, Options, Source},
};
pub use exmon_domain::network_filter::NetworkFilter;
use exmon_domain::observation::Observation;
pub use exmon_protocol::network_filter::FilterPlan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    BpfAndPost,
    PostOnly,
    AllFrames,
}
impl FilterMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::BpfAndPost => "BPF prefilter + exact postfilter",
            Self::PostOnly => "postfilter only (backend BPF unavailable)",
            Self::AllFrames => "all frames; interface/direction postfilter",
        }
    }
}
pub struct FilteredEthernet<S: Source> {
    inner: Ethernet<S>,
    plan: FilterPlan,
    mode: FilterMode,
    selected: Option<u64>,
    active: bool,
    rejected: u64,
}
impl<S: Source> FilteredEthernet<S> {
    /// No live discovery/open. Invalid settings/compiler errors fail before activation.
    pub fn new(
        mut source: S,
        settings: NetworkFilter,
        mut options: Options,
    ) -> Result<Self, Error> {
        if !options.filter.is_empty() {
            return Err(Error::InvalidConfig);
        }
        let plan = FilterPlan::new(settings).map_err(|_| Error::InvalidConfig)?;
        let mode = if plan.bpf().is_empty() {
            FilterMode::AllFrames
        } else {
            match source.validate_filter(plan.bpf()) {
                Ok(()) => {
                    options.filter = plan.bpf().to_owned();
                    FilterMode::BpfAndPost
                }
                Err(Error::Unsupported) => FilterMode::PostOnly,
                Err(error) => return Err(error),
            }
        };
        Ok(Self {
            inner: Ethernet::new(source, options)?,
            plan,
            mode,
            selected: None,
            active: false,
            rejected: 0,
        })
    }
    pub fn plan(&self) -> &FilterPlan {
        &self.plan
    }
    pub fn mode(&self) -> FilterMode {
        self.mode
    }
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn rejected(&self) -> u64 {
        self.rejected
    }
    pub fn interface_name(&self, id: u64) -> Option<&str> {
        self.inner.interface_name(id)
    }
}
impl<S: Source> Adapter for FilteredEthernet<S> {
    fn enumerate(&mut self, limit: usize) -> Result<Vec<Descriptor>, Error> {
        self.inner.enumerate(limit)
    }
    fn open(&mut self, id: u64) -> Result<Descriptor, Error> {
        if self.plan.settings().interface.is_some_and(|i| i != id) {
            return Err(Error::InvalidConfig);
        }
        let d = self.inner.open(id)?;
        self.selected = Some(id);
        Ok(d)
    }
    fn configure(&mut self, snapshot: usize) -> Result<(), Error> {
        self.inner.configure(snapshot)
    }
    fn start(&mut self) -> Result<(), Error> {
        // Preserve a running state if a duplicate start is rejected.
        self.inner.start()?;
        self.active = true;
        self.rejected = 0;
        Ok(())
    }
    fn poll(
        &mut self,
        cancel: &Cancellation,
        items: usize,
        bytes: usize,
    ) -> Result<Vec<Observation>, Error> {
        let mut records = match self.inner.poll(cancel, items, bytes) {
            Ok(records) => records,
            Err(error) => {
                if !matches!(error, Error::InvalidConfig | Error::InvalidState) {
                    let _ = self.inner.stop();
                    self.active = false;
                }
                return Err(error);
            }
        };
        let interface = self.selected.ok_or(Error::InvalidState)?;
        records.retain(|record| {
            let accepted = self.plan.matches(interface, record);
            if !accepted {
                self.rejected = self.rejected.saturating_add(1);
            }
            accepted
        });
        // Do not refill a rejected batch: its input budget still bounds CPU and I/O.
        Ok(records)
    }
    fn stop(&mut self) -> Result<(), Error> {
        let result = self.inner.stop();
        self.active = false;
        result
    }
    fn close(&mut self) -> Result<(), Error> {
        let result = self.inner.close();
        self.active = false;
        self.selected = None;
        result
    }
    fn take_backend_loss(&mut self) -> Option<Option<u64>> {
        self.inner.take_backend_loss()
    }
}
