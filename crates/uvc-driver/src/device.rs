use uvc_core::{CameraId, EngineResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum UsbTransferType {
    Control,
    Isochronous,
    Interrupt,
    Bulk,
    Other(u8),
}

impl UsbTransferType {
    pub fn from_uvc_endpoint_type(value: u8) -> Self {
        match value & 0x03 {
            0x00 => Self::Control,
            0x01 => Self::Isochronous,
            0x02 => Self::Interrupt,
            0x03 => Self::Bulk,
            value => Self::Other(value),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TransferDirection {
    Out,
    In,
}

impl TransferDirection {
    pub fn from_endpoint_address(address: u8) -> Self {
        if address & 0x80 == 0 {
            Self::Out
        } else {
            Self::In
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsbEndpoint {
    address: u8,
    transfer_type: UsbTransferType,
    max_packet_size: u16,
    interval: u8,
}

impl UsbEndpoint {
    pub fn new(
        address: u8,
        transfer_type: UsbTransferType,
        max_packet_size: u16,
        interval: u8,
    ) -> Self {
        Self {
            address,
            transfer_type,
            max_packet_size,
            interval,
        }
    }

    pub fn address(&self) -> u8 {
        self.address
    }

    pub fn direction(&self) -> TransferDirection {
        TransferDirection::from_endpoint_address(self.address)
    }

    pub fn transfer_type(&self) -> UsbTransferType {
        self.transfer_type
    }

    pub fn max_packet_size(&self) -> u16 {
        self.max_packet_size
    }

    pub fn interval(&self) -> u8 {
        self.interval
    }

    pub fn packet_payload_size(&self) -> u16 {
        self.max_packet_size & 0x07ff
    }

    pub fn packets_per_microframe(&self) -> u16 {
        1 + ((self.max_packet_size >> 11) & 0x03)
    }

    pub fn bandwidth_bytes_per_second(&self) -> u32 {
        u32::from(self.packet_payload_size()) * u32::from(self.packets_per_microframe()) * 8000
    }

    pub fn is_in_endpoint(&self) -> bool {
        self.direction() == TransferDirection::In
    }

    pub fn is_iso_in_endpoint(&self) -> bool {
        self.is_in_endpoint() && self.transfer_type == UsbTransferType::Isochronous
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsbInterface {
    interface_number: u8,
    alternate_setting: u8,
    class_code: u8,
    subclass_code: u8,
    protocol_code: u8,
    endpoints: Vec<UsbEndpoint>,
}

impl UsbInterface {
    pub fn new(interface_number: u8, alternate_setting: u8, endpoints: Vec<UsbEndpoint>) -> Self {
        Self::with_class_codes(interface_number, alternate_setting, 0, 0, 0, endpoints)
    }

    pub fn with_class_codes(
        interface_number: u8,
        alternate_setting: u8,
        class_code: u8,
        subclass_code: u8,
        protocol_code: u8,
        endpoints: Vec<UsbEndpoint>,
    ) -> Self {
        Self {
            interface_number,
            alternate_setting,
            class_code,
            subclass_code,
            protocol_code,
            endpoints,
        }
    }

    pub fn interface_number(&self) -> u8 {
        self.interface_number
    }

    pub fn alternate_setting(&self) -> u8 {
        self.alternate_setting
    }

    pub fn class_code(&self) -> u8 {
        self.class_code
    }

    pub fn subclass_code(&self) -> u8 {
        self.subclass_code
    }

    pub fn protocol_code(&self) -> u8 {
        self.protocol_code
    }

    pub fn endpoints(&self) -> &[UsbEndpoint] {
        &self.endpoints
    }

    pub fn bandwidth_bytes_per_second(&self) -> u32 {
        self.endpoints
            .iter()
            .map(UsbEndpoint::bandwidth_bytes_per_second)
            .max()
            .unwrap_or(0)
    }

    pub fn has_iso_in_endpoint(&self) -> bool {
        self.endpoints.iter().any(UsbEndpoint::is_iso_in_endpoint)
    }

    pub fn is_uvc_video_control_interface(&self) -> bool {
        self.class_code == 0x0e && self.subclass_code == 0x01
    }

    pub fn is_uvc_streaming_interface(&self) -> bool {
        self.class_code == 0x0e && self.subclass_code == 0x02
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsbDevice {
    vendor_id: u16,
    product_id: u16,
    bus_number: u8,
    device_address: u8,
    camera_id: Option<CameraId>,
}

impl UsbDevice {
    pub fn new(vendor_id: u16, product_id: u16, bus_number: u8, device_address: u8) -> Self {
        Self {
            vendor_id,
            product_id,
            bus_number,
            device_address,
            camera_id: None,
        }
    }

    pub fn with_camera_id(mut self, camera_id: CameraId) -> Self {
        self.camera_id = Some(camera_id);
        self
    }

    pub fn vendor_id(&self) -> u16 {
        self.vendor_id
    }

    pub fn product_id(&self) -> u16 {
        self.product_id
    }

    pub fn bus_number(&self) -> u8 {
        self.bus_number
    }

    pub fn device_address(&self) -> u8 {
        self.device_address
    }

    pub fn camera_id(&self) -> Option<&CameraId> {
        self.camera_id.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsbDeviceProfile {
    device: UsbDevice,
    interfaces: Vec<UsbInterface>,
}

impl UsbDeviceProfile {
    pub fn new(device: UsbDevice) -> Self {
        Self {
            device,
            interfaces: Vec::new(),
        }
    }

    pub fn with_interfaces(device: UsbDevice, interfaces: Vec<UsbInterface>) -> Self {
        Self { device, interfaces }
    }

    pub fn push_interface(&mut self, interface: UsbInterface) {
        self.interfaces.push(interface);
    }

    pub fn device(&self) -> &UsbDevice {
        &self.device
    }

    pub fn interfaces(&self) -> &[UsbInterface] {
        &self.interfaces
    }

    pub fn uvc_video_control_interfaces(&self) -> impl Iterator<Item = &UsbInterface> {
        self.interfaces
            .iter()
            .filter(|interface| interface.is_uvc_video_control_interface())
    }

    pub fn uvc_streaming_interfaces(&self) -> impl Iterator<Item = &UsbInterface> {
        self.interfaces
            .iter()
            .filter(|interface| interface.is_uvc_streaming_interface())
    }

    pub fn has_uvc_streaming_interface(&self) -> bool {
        self.uvc_streaming_interfaces().next().is_some()
    }

    pub fn select_uvc_streaming_interface(&self) -> Option<&UsbInterface> {
        self.uvc_streaming_interfaces()
            .filter(|interface| interface.has_iso_in_endpoint())
            .max_by_key(|interface| interface.bandwidth_bytes_per_second())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UsbDeviceFilter {
    vendor_id: Option<u16>,
    product_id: Option<u16>,
}

impl UsbDeviceFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn vendor_id(mut self, vendor_id: u16) -> Self {
        self.vendor_id = Some(vendor_id);
        self
    }

    pub fn product_id(mut self, product_id: u16) -> Self {
        self.product_id = Some(product_id);
        self
    }

    pub fn matches(&self, device: &UsbDevice) -> bool {
        self.vendor_id
            .is_none_or(|vendor_id| device.vendor_id() == vendor_id)
            && self
                .product_id
                .is_none_or(|product_id| device.product_id() == product_id)
    }
}

pub fn select_highest_bandwidth_endpoint(endpoints: &[UsbEndpoint]) -> Option<&UsbEndpoint> {
    endpoints
        .iter()
        .filter(|endpoint| endpoint.is_in_endpoint())
        .max_by_key(|endpoint| endpoint.bandwidth_bytes_per_second())
}

pub fn select_highest_bandwidth_interface(interfaces: &[UsbInterface]) -> Option<&UsbInterface> {
    interfaces
        .iter()
        .filter(|interface| interface.has_iso_in_endpoint())
        .max_by_key(|interface| interface.bandwidth_bytes_per_second())
}

pub fn select_uvc_streaming_interface(profile: &UsbDeviceProfile) -> Option<&UsbInterface> {
    profile.select_uvc_streaming_interface()
}

pub fn validate_frame_format_for_endpoint(
    bandwidth_bytes_per_second: u32,
    required_bytes_per_second: u64,
) -> EngineResult<()> {
    let available = u64::from(bandwidth_bytes_per_second);

    if available >= required_bytes_per_second {
        Ok(())
    } else {
        Err(uvc_core::EngineError::InvalidArgument(format!(
            "endpoint bandwidth {available} bytes/s is below required {required_bytes_per_second} bytes/s"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint(address: u8, transfer_type: UsbTransferType, max_packet_size: u16) -> UsbEndpoint {
        UsbEndpoint::new(address, transfer_type, max_packet_size, 1)
    }

    #[test]
    fn endpoint_reports_direction_and_bandwidth() {
        let endpoint = endpoint(0x81, UsbTransferType::Isochronous, 0x1400);

        assert_eq!(endpoint.direction(), TransferDirection::In);
        assert!(endpoint.is_iso_in_endpoint());
        assert_eq!(endpoint.packet_payload_size(), 1024);
        assert_eq!(endpoint.packets_per_microframe(), 3);
        assert_eq!(endpoint.bandwidth_bytes_per_second(), 1024 * 3 * 8000);
    }

    #[test]
    fn selects_highest_bandwidth_in_endpoint() {
        let endpoints = vec![
            endpoint(0x81, UsbTransferType::Isochronous, 512),
            endpoint(0x01, UsbTransferType::Isochronous, 2048),
            endpoint(0x82, UsbTransferType::Isochronous, 1024),
        ];

        assert_eq!(
            select_highest_bandwidth_endpoint(&endpoints).map(UsbEndpoint::address),
            Some(0x82)
        );
    }

    #[test]
    fn selects_interface_with_iso_in_endpoint_and_highest_bandwidth() {
        let low = UsbInterface::new(
            1,
            1,
            vec![endpoint(0x81, UsbTransferType::Isochronous, 512)],
        );
        let out_only = UsbInterface::new(
            1,
            2,
            vec![endpoint(0x01, UsbTransferType::Isochronous, 2048)],
        );
        let high = UsbInterface::new(
            1,
            3,
            vec![endpoint(0x81, UsbTransferType::Isochronous, 1024)],
        );

        assert_eq!(
            select_highest_bandwidth_interface(&[low.clone(), out_only, high.clone()]),
            Some(&high)
        );
    }

    #[test]
    fn selects_uvc_streaming_interface_with_highest_bandwidth() {
        let control = UsbInterface::with_class_codes(1, 0, 0x0e, 0x01, 0, Vec::new());
        let low_stream = UsbInterface::with_class_codes(
            1,
            1,
            0x0e,
            0x02,
            0,
            vec![endpoint(0x81, UsbTransferType::Isochronous, 512)],
        );
        let non_streaming = UsbInterface::with_class_codes(
            1,
            2,
            0xff,
            0x00,
            0,
            vec![endpoint(0x81, UsbTransferType::Isochronous, 2048)],
        );
        let high_stream = UsbInterface::with_class_codes(
            1,
            3,
            0x0e,
            0x02,
            0,
            vec![endpoint(0x81, UsbTransferType::Isochronous, 1024)],
        );
        let profile = UsbDeviceProfile::with_interfaces(
            UsbDevice::new(0x1234, 0x5678, 1, 2),
            vec![
                control,
                low_stream.clone(),
                non_streaming,
                high_stream.clone(),
            ],
        );

        assert!(profile.has_uvc_streaming_interface());
        assert_eq!(profile.uvc_video_control_interfaces().count(), 1);
        assert_eq!(profile.uvc_streaming_interfaces().count(), 2);
        assert_eq!(select_uvc_streaming_interface(&profile), Some(&high_stream));
    }

    #[test]
    fn device_filter_matches_vendor_and_product() {
        let filter = UsbDeviceFilter::new().vendor_id(0x1234).product_id(0x5678);
        let device = UsbDevice::new(0x1234, 0x5678, 1, 2);

        assert!(filter.matches(&device));
        assert!(!UsbDeviceFilter::new().vendor_id(0x1111).matches(&device));
    }
}
