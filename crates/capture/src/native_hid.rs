//! Exact-path HIDAPI access, without serial-number or VID/PID fallback opens.
use crate::{
    adapter::Error,
    hid::{Device, Source},
};
use std::ffi::CString;

#[derive(Default)]
pub struct NativeHid {
    api: Option<hidapi::HidApi>,
    paths: Vec<CString>,
    devices: Vec<Device>,
    handle: Option<hidapi::HidDevice>,
}
fn map_error(error: hidapi::HidError) -> Error {
    // Upstream errors vary by OS. Classify internally, never emit their text.
    let text = error.to_string().to_ascii_lowercase();
    if text.contains("permission") || text.contains("access is denied") {
        Error::PermissionDenied
    } else if text.contains("disconnected")
        || text.contains("not connected")
        || text.contains("no such device")
    {
        Error::Disconnected
    } else {
        Error::Backend
    }
}
impl Source for NativeHid {
    fn enumerate(&mut self, maximum: usize) -> Result<Vec<Device>, Error> {
        if self.handle.is_some() {
            return Err(Error::InvalidState);
        }
        let api = hidapi::HidApi::new().map_err(map_error)?;
        let mut paths = Vec::new();
        let mut devices = Vec::new();
        for info in api
            .device_list()
            .filter(|i| matches!(i.bus_type(), hidapi::BusType::Usb))
            .take(maximum)
        {
            if info.path().to_bytes().len() > 4096 {
                return Err(Error::Backend);
            }
            paths.push(info.path().to_owned());
            devices.push(Device {
                vendor_id: info.vendor_id(),
                product_id: info.product_id(),
                usage_page: info.usage_page(),
                usage: info.usage(),
            });
        }
        self.api = Some(api);
        self.paths = paths;
        self.devices = devices.clone();
        Ok(devices)
    }
    fn open_selected(&mut self, index: usize) -> Result<(), Error> {
        self.close();
        let path = self.paths.get(index).ok_or(Error::InvalidConfig)?;
        let expected = self.devices.get(index).ok_or(Error::InvalidConfig)?;
        let api = self.api.as_mut().ok_or(Error::InvalidState)?;
        api.refresh_devices().map_err(map_error)?;
        let matches = |info: &hidapi::DeviceInfo| {
            info.vendor_id() == expected.vendor_id
                && info.product_id() == expected.product_id
                && info.usage_page() == expected.usage_page
                && info.usage() == expected.usage
        };
        if !api
            .device_list()
            .any(|info| info.path() == path.as_c_str() && matches(info))
        {
            return Err(Error::Disconnected);
        }
        let handle = api.open_path(path).map_err(map_error)?;
        if !matches(&handle.get_device_info().map_err(map_error)?) {
            return Err(Error::Disconnected);
        }
        self.handle = Some(handle);
        Ok(())
    }
    fn read(&mut self, bytes: &mut [u8], timeout_ms: i32) -> Result<usize, Error> {
        self.handle
            .as_ref()
            .ok_or(Error::InvalidState)?
            .read_timeout(bytes, timeout_ms)
            .map_err(map_error)
    }
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        self.handle
            .as_ref()
            .ok_or(Error::InvalidState)?
            .write(bytes)
            .map_err(map_error)
    }
    fn close(&mut self) {
        self.handle = None;
    }
}
