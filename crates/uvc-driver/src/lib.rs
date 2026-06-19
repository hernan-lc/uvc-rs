#[cfg(feature = "rusb")]
pub mod iso;
#[cfg(feature = "rusb")]
pub mod session;

pub mod backend;
pub mod descriptor;
pub mod device;
pub mod fake;
pub mod transfer;

#[cfg(feature = "rusb")]
pub use backend::RusbUsbBackend;
pub use backend::{NoopUsbBackend, UsbBackend};
pub use descriptor::{
    DescriptorHeader, EndpointDescriptor, StreamingDescriptor, StreamingDescriptorKind, UvcFormat,
    UvcFormatType, UvcFrame, UvcStreamCollection, UvcStreamInterface,
};
pub use device::{
    TransferDirection as UsbTransferDirection, UsbDevice, UsbDeviceFilter, UsbDeviceProfile,
    UsbEndpoint, UsbInterface, UsbTransferType, select_highest_bandwidth_endpoint,
    select_highest_bandwidth_interface, select_uvc_streaming_interface,
    validate_frame_format_for_endpoint,
};
pub use fake::{FakeCameraPipeline, FakeFrameGenerator, FakeMultiCameraEngine};
#[cfg(feature = "rusb")]
pub use iso::{CompletedIsoTransfer, IsoPacketLayout, LibusbIsochronousLoop};
#[cfg(feature = "rusb")]
pub use session::{RusbTransferReader, RusbUsbDeviceSession};
pub use transfer::{
    CompletedTransfer, TransferBuffer, TransferDirection, TransferKind, TransferLoop,
    TransferRequest,
};

pub use uvc_core::{CameraConfig, CameraId, EngineResult, FrameFormat, FrameReceiver, FrameSender};
