pub mod descriptor;
pub mod fake;

pub use descriptor::{
    DescriptorHeader, EndpointDescriptor, StreamingDescriptor, StreamingDescriptorKind, UvcFormat,
    UvcFormatType, UvcFrame, UvcStreamCollection, UvcStreamInterface,
};
pub use fake::{FakeCameraPipeline, FakeFrameGenerator, FakeMultiCameraEngine};

pub use uvc_core::{CameraConfig, CameraId, EngineResult, FrameFormat, FrameReceiver, FrameSender};
