use std::os::raw::c_int;

use uvc_core::{CameraId, EngineError, EngineResult};

#[cfg(all(feature = "android", unix))]
use rusb::{Context, DeviceHandle, UsbContext};

pub type NativeFileDescriptor = c_int;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AndroidFdHandle {
    fd: NativeFileDescriptor,
}

impl AndroidFdHandle {
    pub fn new(fd: NativeFileDescriptor) -> EngineResult<Self> {
        if fd < 0 {
            return Err(EngineError::InvalidArgument(format!(
                "Android USB file descriptor must be non-negative, got {fd}"
            )));
        }

        Ok(Self { fd })
    }

    pub fn fd(self) -> NativeFileDescriptor {
        self.fd
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AndroidUsbDevice {
    fd: AndroidFdHandle,
    vendor_id: u16,
    product_id: u16,
    camera_id: CameraId,
}

impl AndroidUsbDevice {
    pub fn new(
        fd: NativeFileDescriptor,
        vendor_id: u16,
        product_id: u16,
        camera_id: CameraId,
    ) -> EngineResult<Self> {
        Ok(Self {
            fd: AndroidFdHandle::new(fd)?,
            vendor_id,
            product_id,
            camera_id,
        })
    }

    pub fn fd(self) -> AndroidFdHandle {
        self.fd
    }

    pub fn vendor_id(&self) -> u16 {
        self.vendor_id
    }

    pub fn product_id(&self) -> u16 {
        self.product_id
    }

    pub fn camera_id(&self) -> &CameraId {
        &self.camera_id
    }
}

#[cfg(all(feature = "android", unix))]
pub struct AndroidUsbDeviceConnection {
    device: AndroidUsbDevice,
    context: Context,
    handle: DeviceHandle<Context>,
}

#[cfg(all(feature = "android", unix))]
impl AndroidUsbDeviceConnection {
    pub fn open(device: AndroidUsbDevice) -> EngineResult<Self> {
        let fd = device.clone().fd().fd();
        let context = Context::new().map_err(rusb_error)?;
        let handle =
            unsafe { context.open_device_with_fd(fd) }.map_err(rusb_error)?;

        Ok(Self {
            device,
            context,
            handle,
        })
    }

    pub fn device(&self) -> &AndroidUsbDevice {
        &self.device
    }

    pub fn raw_context(&self) -> *mut libusb1_sys::libusb_context {
        self.context.as_raw()
    }

    pub fn raw_handle(&self) -> *mut libusb1_sys::libusb_device_handle {
        self.handle.as_raw()
    }
}

#[cfg(all(feature = "android", not(unix)))]
pub struct AndroidUsbDeviceConnection {
    _private: (),
}

#[cfg(all(feature = "android", not(unix)))]
impl AndroidUsbDeviceConnection {
    pub fn open(_device: AndroidUsbDevice) -> EngineResult<Self> {
        Err(EngineError::Backend(
            "Android file-descriptor wrapping requires a unix target".to_owned(),
        ))
    }
}

#[cfg(all(feature = "android", unix))]
fn rusb_error(error: rusb::Error) -> EngineError {
    EngineError::Backend(format!("rusb/libusb error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_fd_handle_rejects_negative_fds() {
        assert!(AndroidFdHandle::new(-1).is_err());
        assert_eq!(AndroidFdHandle::new(42).unwrap().fd(), 42);
    }

    #[test]
    fn android_usb_device_tracks_identity() {
        let device =
            AndroidUsbDevice::new(42, 0x1234, 0x5678, CameraId::new("usb-cam-1").unwrap()).unwrap();

        assert_eq!(device.vendor_id(), 0x1234);
        assert_eq!(device.product_id(), 0x5678);
        assert_eq!(device.camera_id().as_str(), "usb-cam-1");
    }
}
