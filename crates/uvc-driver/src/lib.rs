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
    TransferDirection as UsbTransferDirection, UsbDevice, UsbDeviceFilter, UsbEndpoint,
    UsbInterface, UsbTransferType, select_highest_bandwidth_endpoint,
    select_highest_bandwidth_interface, validate_frame_format_for_endpoint,
};
pub use fake::{FakeCameraPipeline, FakeFrameGenerator, FakeMultiCameraEngine};
pub use transfer::{CompletedTransfer, TransferDirection, TransferKind, TransferRequest};

pub use uvc_core::{CameraConfig, CameraId, EngineResult, FrameFormat, FrameReceiver, FrameSender};
