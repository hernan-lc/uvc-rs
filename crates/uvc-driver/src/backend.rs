use uvc_core::EngineResult;

use crate::UsbDevice;

#[cfg(feature = "rusb")]
use crate::{UsbDeviceFilter, UsbDeviceProfile, UsbEndpoint, UsbInterface, UsbTransferType};
#[cfg(feature = "rusb")]
use rusb::{ConfigDescriptor, EndpointDescriptor, TransferType, UsbContext};
#[cfg(feature = "rusb")]
use uvc_core::EngineError;

pub trait UsbBackend {
    fn devices(&mut self) -> EngineResult<Vec<UsbDevice>>;
}

#[derive(Clone, Debug, Default)]
pub struct NoopUsbBackend {
    devices: Vec<UsbDevice>,
}

impl NoopUsbBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_devices(devices: Vec<UsbDevice>) -> Self {
        Self { devices }
    }

    pub fn devices(&self) -> &[UsbDevice] {
        &self.devices
    }
}

impl UsbBackend for NoopUsbBackend {
    fn devices(&mut self) -> EngineResult<Vec<UsbDevice>> {
        Ok(self.devices.clone())
    }
}

#[cfg(feature = "rusb")]
#[derive(Debug)]
pub struct RusbUsbBackend {
    context: rusb::Context,
    filter: UsbDeviceFilter,
}

#[cfg(feature = "rusb")]
impl RusbUsbBackend {
    pub fn new() -> EngineResult<Self> {
        Self::with_filter(UsbDeviceFilter::new())
    }

    pub fn with_filter(filter: UsbDeviceFilter) -> EngineResult<Self> {
        let context = rusb::Context::new().map_err(rusb_error)?;

        Ok(Self { context, filter })
    }

    pub fn filter(&self) -> &UsbDeviceFilter {
        &self.filter
    }

    pub fn discover_devices(&mut self) -> EngineResult<Vec<UsbDeviceProfile>> {
        let devices = self.context.devices().map_err(rusb_error)?;
        let mut result = Vec::new();

        for device in devices.iter() {
            let descriptor = device.device_descriptor().map_err(rusb_error)?;
            let usb_device = UsbDevice::new(
                descriptor.vendor_id(),
                descriptor.product_id(),
                device.bus_number(),
                device.address(),
            );

            if !self.filter.matches(&usb_device) {
                continue;
            }

            let mut profile = UsbDeviceProfile::new(usb_device);

            if let Ok(config) = device.active_config_descriptor() {
                parse_config_descriptor(&mut profile, &config);
            }

            result.push(profile);
        }

        Ok(result)
    }
}

#[cfg(feature = "rusb")]
impl UsbBackend for RusbUsbBackend {
    fn devices(&mut self) -> EngineResult<Vec<UsbDevice>> {
        let devices = self.context.devices().map_err(rusb_error)?;
        let mut result = Vec::new();

        for device in devices.iter() {
            let descriptor = device.device_descriptor().map_err(rusb_error)?;
            let usb_device = UsbDevice::new(
                descriptor.vendor_id(),
                descriptor.product_id(),
                device.bus_number(),
                device.address(),
            );

            if self.filter.matches(&usb_device) {
                result.push(usb_device);
            }
        }

        Ok(result)
    }
}

#[cfg(feature = "rusb")]
fn parse_config_descriptor(profile: &mut UsbDeviceProfile, config: &ConfigDescriptor) {
    for interface in config.interfaces() {
        for descriptor in interface.descriptors() {
            let endpoints = descriptor
                .endpoint_descriptors()
                .map(endpoint_from_descriptor)
                .collect::<Vec<_>>();
            let usb_interface = UsbInterface::with_class_codes(
                descriptor.interface_number(),
                descriptor.setting_number(),
                descriptor.class_code(),
                descriptor.sub_class_code(),
                descriptor.protocol_code(),
                endpoints,
            );

            profile.push_interface(usb_interface);
        }
    }
}

#[cfg(feature = "rusb")]
fn endpoint_from_descriptor(descriptor: EndpointDescriptor) -> UsbEndpoint {
    UsbEndpoint::new(
        descriptor.address(),
        transfer_type_from_rusb(descriptor.transfer_type()),
        descriptor.max_packet_size(),
        descriptor.interval(),
    )
}

#[cfg(feature = "rusb")]
fn transfer_type_from_rusb(transfer_type: TransferType) -> UsbTransferType {
    match transfer_type {
        TransferType::Control => UsbTransferType::Control,
        TransferType::Isochronous => UsbTransferType::Isochronous,
        TransferType::Interrupt => UsbTransferType::Interrupt,
        TransferType::Bulk => UsbTransferType::Bulk,
    }
}

#[cfg(feature = "rusb")]
fn rusb_error(error: rusb::Error) -> EngineError {
    EngineError::Backend(format!("rusb/libusb error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UsbDevice;

    #[test]
    fn noop_backend_returns_configured_devices() {
        let devices = vec![UsbDevice::new(0x1234, 0x5678, 1, 2)];
        let backend = NoopUsbBackend::with_devices(devices.clone());

        assert_eq!(backend.devices(), devices.as_slice());
    }
}
