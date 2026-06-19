use std::time::Duration;

use rusb::{Context, Device, DeviceHandle, UsbContext};
use uvc_core::{EngineError, EngineResult};

use crate::{
    CompletedTransfer, TransferBuffer, TransferLoop, TransferRequest, UsbEndpoint, UsbTransferType,
};

pub struct RusbUsbDeviceSession {
    _device: Device<Context>,
    handle: DeviceHandle<Context>,
    interface_number: u8,
    alternate_setting: u8,
    claimed: bool,
}

impl RusbUsbDeviceSession {
    pub fn open(
        device: Device<Context>,
        interface_number: u8,
        alternate_setting: u8,
    ) -> EngineResult<Self> {
        let handle = device.open().map_err(rusb_error)?;
        let mut session = Self {
            _device: device,
            handle,
            interface_number,
            alternate_setting,
            claimed: false,
        };

        session.claim_interface()?;
        session.set_alternate_setting()?;

        Ok(session)
    }

    pub fn interface_number(&self) -> u8 {
        self.interface_number
    }

    pub fn alternate_setting(&self) -> u8 {
        self.alternate_setting
    }

    pub fn claim_interface(&mut self) -> EngineResult<()> {
        if !self.claimed {
            self.handle
                .claim_interface(self.interface_number)
                .map_err(rusb_error)?;
            self.claimed = true;
        }

        Ok(())
    }

    pub fn set_alternate_setting(&self) -> EngineResult<()> {
        self.handle
            .set_alternate_setting(self.interface_number, self.alternate_setting)
            .map_err(rusb_error)?;

        Ok(())
    }

    pub fn release_interface(&mut self) -> EngineResult<()> {
        if self.claimed {
            self.handle
                .release_interface(self.interface_number)
                .map_err(rusb_error)?;
            self.claimed = false;
        }

        Ok(())
    }

    pub fn raw_context(&self) -> *mut libusb1_sys::libusb_context {
        self.handle.context().as_raw()
    }

    pub fn raw_handle(&self) -> *mut libusb1_sys::libusb_device_handle {
        self.handle.as_raw()
    }

    pub fn read_endpoint(
        &self,
        endpoint: &UsbEndpoint,
        buffer: &mut [u8],
        timeout: Duration,
    ) -> EngineResult<usize> {
        if endpoint.address() & 0x80 == 0 {
            return Err(EngineError::InvalidArgument(format!(
                "endpoint 0x{:02x} is not an IN endpoint",
                endpoint.address()
            )));
        }

        match endpoint.transfer_type() {
            UsbTransferType::Bulk => self
                .handle
                .read_bulk(endpoint.address(), buffer, timeout)
                .map_err(rusb_error),
            UsbTransferType::Interrupt => self
                .handle
                .read_interrupt(endpoint.address(), buffer, timeout)
                .map_err(rusb_error),
            UsbTransferType::Isochronous => Err(EngineError::Backend(
                "rusb sync API does not expose isochronous transfers; use the libusb async loop path".to_owned(),
            )),
            UsbTransferType::Control => Err(EngineError::InvalidArgument(
                "control transfers must use rusb control request APIs".to_owned(),
            )),
            UsbTransferType::Other(value) => Err(EngineError::InvalidArgument(format!(
                "unsupported endpoint transfer type {value}"
            ))),
        }
    }
}

impl Drop for RusbUsbDeviceSession {
    fn drop(&mut self) {
        let _ = self.release_interface();
    }
}

pub struct RusbTransferReader<'a> {
    session: &'a RusbUsbDeviceSession,
    endpoint: UsbEndpoint,
    buffer: TransferBuffer,
}

impl<'a> RusbTransferReader<'a> {
    pub fn new(
        session: &'a RusbUsbDeviceSession,
        endpoint: UsbEndpoint,
        timeout: Duration,
    ) -> EngineResult<Self> {
        let request = TransferRequest::iso_in(
            endpoint.address(),
            usize::from(endpoint.packet_payload_size())
                * usize::from(endpoint.packets_per_microframe()),
            timeout,
        )?;
        let buffer = TransferBuffer::new(request);

        Ok(Self {
            session,
            endpoint,
            buffer,
        })
    }

    pub fn endpoint(&self) -> &UsbEndpoint {
        &self.endpoint
    }

    pub fn buffer(&self) -> &TransferBuffer {
        &self.buffer
    }

    pub fn buffer_mut(&mut self) -> &mut TransferBuffer {
        &mut self.buffer
    }
}

impl TransferLoop for RusbTransferReader<'_> {
    fn poll(&mut self) -> EngineResult<Option<CompletedTransfer>> {
        let timeout = self.buffer.request().timeout();
        let transferred_len =
            self.session
                .read_endpoint(&self.endpoint, self.buffer.data_mut(), timeout)?;

        Ok(Some(CompletedTransfer::new(
            self.endpoint.address(),
            transferred_len,
        )))
    }
}

fn rusb_error(error: rusb::Error) -> EngineError {
    EngineError::Backend(format!("rusb/libusb error: {error}"))
}
