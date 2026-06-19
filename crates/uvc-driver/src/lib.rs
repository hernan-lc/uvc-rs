#[cfg(feature = "rusb")]
pub mod iso;
#[cfg(feature = "rusb")]
pub mod session;

pub mod backend;
pub mod decode;
pub mod descriptor;
pub mod device;
pub mod fake;
pub mod packet;
pub mod transfer;

#[cfg(feature = "rusb")]
pub use backend::RusbUsbBackend;
pub use backend::{NoopUsbBackend, UsbBackend};
pub use decode::{
    DecodedFrameSinkAdapter, FrameDecoder, MjpegToRgbaDecoder, Nv12ToRgbaDecoder, YuyvToRgbaDecoder,
};
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
pub use iso::{CompletedIsoTransfer, IsoPacketLayout, LibusbIsochronousLoop, MjpegIsoFrameLoop};
pub use packet::{
    MjpegFrameAssembler, MjpegFrameSinkAdapter, UvcPacketAssembler, UvcPayloadHeader,
    is_mjpeg_frame,
};

pub use uvc_core::{CameraConfig, CameraId, EngineResult, FrameFormat, FrameReceiver, FrameSender};
