use std::fmt;

use uvc_core::{EngineError, EngineResult};

const DESC_TYPE_CLASS_SPECIFIC_VIDEO: u8 = 0x24;
const DESC_TYPE_ENDPOINT: u8 = 0x05;

const DESC_SUBTYPE_FORMAT_UNCOMPRESSED: u8 = 0x04;
const DESC_SUBTYPE_FRAME_UNCOMPRESSED: u8 = 0x05;
const DESC_SUBTYPE_FORMAT_MJPEG: u8 = 0x06;
const DESC_SUBTYPE_FRAME_MJPEG: u8 = 0x07;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum UvcFormatType {
    Mjpeg,
    Uncompressed,
    Unknown(u8),
}

impl UvcFormatType {
    pub fn from_subtype(subtype: u8) -> Self {
        match subtype {
            DESC_SUBTYPE_FORMAT_MJPEG => Self::Mjpeg,
            DESC_SUBTYPE_FORMAT_UNCOMPRESSED => Self::Uncompressed,
            value => Self::Unknown(value),
        }
    }
}

impl fmt::Display for UvcFormatType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mjpeg => f.write_str("mjpeg"),
            Self::Uncompressed => f.write_str("uncompressed"),
            Self::Unknown(value) => write!(f, "unknown({value})"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum StreamingDescriptorKind {
    InputHeader,
    Format,
    Frame,
    StillImageFrame,
    ColorFormat,
    Unknown(u8),
}

impl StreamingDescriptorKind {
    pub fn from_subtype(subtype: u8) -> Self {
        match subtype {
            0x01 => Self::InputHeader,
            0x02 | 0x04 | 0x06 => Self::Format,
            value if is_frame_subtype(value) => Self::Frame,
            0x03 => Self::StillImageFrame,
            0x0a => Self::ColorFormat,
            value => Self::Unknown(value),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DescriptorHeader {
    length: usize,
    descriptor_type: u8,
    descriptor_subtype: Option<u8>,
}

impl DescriptorHeader {
    pub fn parse_at(input: &[u8], offset: usize) -> EngineResult<Self> {
        if offset + 2 > input.len() {
            return Err(EngineError::InvalidArgument(format!(
                "descriptor header at offset {offset} exceeds input length {}",
                input.len()
            )));
        }

        let length = usize::from(input[offset]);
        let descriptor_type = input[offset + 1];

        if length < 2 {
            return Err(EngineError::InvalidArgument(format!(
                "descriptor at offset {offset} has invalid length {length}"
            )));
        }

        if offset + length > input.len() {
            return Err(EngineError::InvalidArgument(format!(
                "descriptor at offset {offset} with length {length} exceeds input length {}",
                input.len()
            )));
        }

        let descriptor_subtype = (length > 2).then_some(input[offset + 2]);

        Ok(Self {
            length,
            descriptor_type,
            descriptor_subtype,
        })
    }

    pub fn length(self) -> usize {
        self.length
    }

    pub fn descriptor_type(self) -> u8 {
        self.descriptor_type
    }

    pub fn descriptor_subtype(self) -> Option<u8> {
        self.descriptor_subtype
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamingDescriptor {
    header: DescriptorHeader,
    kind: StreamingDescriptorKind,
    payload: Vec<u8>,
}

impl StreamingDescriptor {
    pub fn parse(input: &[u8], offset: usize) -> EngineResult<Self> {
        let header = DescriptorHeader::parse_at(input, offset)?;

        if header.descriptor_type != DESC_TYPE_CLASS_SPECIFIC_VIDEO {
            return Err(EngineError::InvalidArgument(format!(
                "expected class-specific video descriptor 0x{DESC_TYPE_CLASS_SPECIFIC_VIDEO:02x}, got 0x{:02x}",
                header.descriptor_type
            )));
        }

        let subtype = header.descriptor_subtype.ok_or_else(|| {
            EngineError::InvalidArgument("streaming descriptor is missing subtype".to_owned())
        })?;
        let payload_start = offset + 3;
        let payload_end = offset + header.length;

        Ok(Self {
            header,
            kind: StreamingDescriptorKind::from_subtype(subtype),
            payload: input[payload_start..payload_end].to_vec(),
        })
    }

    pub fn header(&self) -> DescriptorHeader {
        self.header
    }

    pub fn kind(&self) -> StreamingDescriptorKind {
        self.kind
    }

    pub fn subtype(&self) -> u8 {
        self.header.descriptor_subtype.unwrap_or(0)
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UvcFormat {
    format_index: u8,
    number_of_frame_descriptors: u8,
    format_type: UvcFormatType,
    guid_format_type: Option<[u8; 16]>,
    bits_per_pixel: u8,
    default_frame_index: u8,
    aspect_ratio: Option<(u8, u8)>,
    frames: Vec<UvcFrame>,
}

impl UvcFormat {
    pub fn parse(descriptor: &StreamingDescriptor) -> EngineResult<Self> {
        let payload = descriptor.payload();
        let subtype = descriptor.subtype();

        if payload.len() < 2 {
            return Err(EngineError::InvalidArgument(format!(
                "format descriptor {subtype} is too short: {} bytes",
                payload.len()
            )));
        }

        let (format_type, guid_format_type, bits_offset, default_offset, aspect_offset) =
            match subtype {
                DESC_SUBTYPE_FORMAT_UNCOMPRESSED => {
                    if payload.len() < 22 {
                        return Err(EngineError::InvalidArgument(
                            "uncompressed format descriptor is shorter than 22 bytes".to_owned(),
                        ));
                    }

                    let mut guid = [0u8; 16];
                    guid.copy_from_slice(&payload[2..18]);

                    (
                        UvcFormatType::Uncompressed,
                        Some(guid),
                        18,
                        19,
                        Some((20, 21)),
                    )
                }
                DESC_SUBTYPE_FORMAT_MJPEG => (UvcFormatType::Mjpeg, None, 2, 3, Some((4, 5))),
                value => (
                    UvcFormatType::Unknown(value),
                    None,
                    payload.len().min(2),
                    0,
                    None,
                ),
            };

        let bits_per_pixel = payload.get(bits_offset).copied().ok_or_else(|| {
            EngineError::InvalidArgument("format descriptor missing bits per pixel".to_owned())
        })?;
        let default_frame_index = payload.get(default_offset).copied().unwrap_or(0);
        let aspect_ratio = aspect_offset.and_then(|(x, y)| {
            payload
                .get(x)
                .copied()
                .zip(payload.get(y).copied())
                .map(|(width, height)| (width, height))
        });

        Ok(Self {
            format_index: payload[0],
            number_of_frame_descriptors: payload[1],
            format_type,
            guid_format_type,
            bits_per_pixel,
            default_frame_index,
            aspect_ratio,
            frames: Vec::new(),
        })
    }

    pub fn format_index(&self) -> u8 {
        self.format_index
    }

    pub fn number_of_frame_descriptors(&self) -> u8 {
        self.number_of_frame_descriptors
    }

    pub fn format_type(&self) -> UvcFormatType {
        self.format_type
    }

    pub fn guid_format_type(&self) -> Option<[u8; 16]> {
        self.guid_format_type
    }

    pub fn bits_per_pixel(&self) -> u8 {
        self.bits_per_pixel
    }

    pub fn default_frame_index(&self) -> u8 {
        self.default_frame_index
    }

    pub fn aspect_ratio(&self) -> Option<(u8, u8)> {
        self.aspect_ratio
    }

    pub fn frames(&self) -> &[UvcFrame] {
        &self.frames
    }

    pub fn push_frame(&mut self, frame: UvcFrame) {
        self.frames.push(frame);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UvcFrame {
    frame_index: u8,
    capabilities: u8,
    width: u16,
    height: u16,
    min_bit_rate: u32,
    max_bit_rate: u32,
    default_frame_interval: u32,
    frame_intervals: Vec<u32>,
}

impl UvcFrame {
    pub fn parse(descriptor: &StreamingDescriptor) -> EngineResult<Self> {
        let payload = descriptor.payload();

        if payload.len() < 18 {
            return Err(EngineError::InvalidArgument(format!(
                "frame descriptor {} is too short: {} bytes",
                descriptor.subtype(),
                payload.len()
            )));
        }

        let frame_intervals = payload[18..]
            .chunks_exact(4)
            .map(|chunk| le_u32(chunk))
            .collect();

        Ok(Self {
            frame_index: payload[0],
            capabilities: payload[1],
            width: le_u16(&payload[2..4]),
            height: le_u16(&payload[4..6]),
            min_bit_rate: le_u32(&payload[6..10]),
            max_bit_rate: le_u32(&payload[10..14]),
            default_frame_interval: le_u32(&payload[14..18]),
            frame_intervals,
        })
    }

    pub fn frame_index(&self) -> u8 {
        self.frame_index
    }

    pub fn capabilities(&self) -> u8 {
        self.capabilities
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn min_bit_rate(&self) -> u32 {
        self.min_bit_rate
    }

    pub fn max_bit_rate(&self) -> u32 {
        self.max_bit_rate
    }

    pub fn default_frame_interval(&self) -> u32 {
        self.default_frame_interval
    }

    pub fn frame_intervals(&self) -> &[u32] {
        &self.frame_intervals
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EndpointDescriptor {
    address: u8,
    attributes: u8,
    max_packet_size: u16,
    interval: u8,
    extra: Vec<u8>,
}

impl EndpointDescriptor {
    pub fn parse(input: &[u8], offset: usize) -> EngineResult<Self> {
        let header = DescriptorHeader::parse_at(input, offset)?;

        if header.descriptor_type != DESC_TYPE_ENDPOINT {
            return Err(EngineError::InvalidArgument(format!(
                "expected endpoint descriptor 0x{DESC_TYPE_ENDPOINT:02x}, got 0x{:02x}",
                header.descriptor_type
            )));
        }

        if header.length < 7 {
            return Err(EngineError::InvalidArgument(format!(
                "endpoint descriptor at offset {offset} is shorter than 7 bytes"
            )));
        }

        let max_packet_size = le_u16(&input[offset + 4..offset + 6]);
        let extra_start = offset + 7;
        let extra_end = offset + header.length;

        Ok(Self {
            address: input[offset + 2],
            attributes: input[offset + 3],
            max_packet_size,
            interval: input[offset + 6],
            extra: input[extra_start..extra_end].to_vec(),
        })
    }

    pub fn address(&self) -> u8 {
        self.address
    }

    pub fn attributes(&self) -> u8 {
        self.attributes
    }

    pub fn max_packet_size(&self) -> u16 {
        self.max_packet_size
    }

    pub fn interval(&self) -> u8 {
        self.interval
    }

    pub fn extra(&self) -> &[u8] {
        &self.extra
    }

    pub fn bandwidth_bytes_per_second(&self) -> u32 {
        let payload_size = u32::from(self.max_packet_size & 0x07ff);
        let packets_per_microframe = 1 + u32::from((self.max_packet_size >> 11) & 0x03);
        payload_size * packets_per_microframe * 8000
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UvcStreamInterface {
    interface_number: u8,
    alternate_setting: u8,
    class_specific: Vec<StreamingDescriptor>,
    endpoints: Vec<EndpointDescriptor>,
    formats: Vec<UvcFormat>,
}

impl UvcStreamInterface {
    pub fn parse(interface_number: u8, alternate_setting: u8, input: &[u8]) -> EngineResult<Self> {
        let mut class_specific = Vec::new();
        let mut endpoints = Vec::new();
        let mut offset = 0;

        while offset < input.len() {
            let header = DescriptorHeader::parse_at(input, offset)?;

            match header.descriptor_type {
                DESC_TYPE_CLASS_SPECIFIC_VIDEO => {
                    class_specific.push(StreamingDescriptor::parse(input, offset)?);
                }
                DESC_TYPE_ENDPOINT => {
                    endpoints.push(EndpointDescriptor::parse(input, offset)?);
                }
                _ => {}
            }

            offset += header.length;
        }

        let formats = build_formats(&class_specific);

        Ok(Self {
            interface_number,
            alternate_setting,
            class_specific,
            endpoints,
            formats,
        })
    }

    pub fn interface_number(&self) -> u8 {
        self.interface_number
    }

    pub fn alternate_setting(&self) -> u8 {
        self.alternate_setting
    }

    pub fn class_specific(&self) -> &[StreamingDescriptor] {
        &self.class_specific
    }

    pub fn endpoints(&self) -> &[EndpointDescriptor] {
        &self.endpoints
    }

    pub fn formats(&self) -> &[UvcFormat] {
        &self.formats
    }

    pub fn endpoint_bandwidth_bytes_per_second(&self) -> u32 {
        self.endpoints
            .iter()
            .map(EndpointDescriptor::bandwidth_bytes_per_second)
            .max()
            .unwrap_or(0)
    }

    pub fn format(&self, format_index: u8) -> Option<&UvcFormat> {
        self.formats
            .iter()
            .find(|format| format.format_index() == format_index)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UvcStreamCollection {
    interfaces: Vec<UvcStreamInterface>,
}

impl UvcStreamCollection {
    pub fn new(interfaces: Vec<UvcStreamInterface>) -> Self {
        Self { interfaces }
    }

    pub fn interfaces(&self) -> &[UvcStreamInterface] {
        &self.interfaces
    }

    pub fn select_alternate_setting(&self) -> Option<&UvcStreamInterface> {
        self.interfaces
            .iter()
            .max_by_key(|interface| interface.endpoint_bandwidth_bytes_per_second())
    }
}

fn is_frame_subtype(subtype: u8) -> bool {
    matches!(
        subtype,
        DESC_SUBTYPE_FRAME_UNCOMPRESSED | DESC_SUBTYPE_FRAME_MJPEG
    )
}

fn build_formats(descriptors: &[StreamingDescriptor]) -> Vec<UvcFormat> {
    let mut formats = Vec::new();
    let mut current_format = None;

    for descriptor in descriptors {
        match descriptor.kind() {
            StreamingDescriptorKind::Format => {
                if let Ok(format) = UvcFormat::parse(descriptor) {
                    current_format = Some(formats.len());
                    formats.push(format);
                }
            }
            StreamingDescriptorKind::Frame => {
                if let (Some(index), Ok(frame)) = (current_format, UvcFrame::parse(descriptor)) {
                    formats[index].push_frame(frame);
                }
            }
            _ => current_format = None,
        }
    }

    formats
}

fn le_u16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn le_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class_specific_descriptor(subtype: u8, payload: &[u8]) -> Vec<u8> {
        let mut descriptor = vec![
            (payload.len() + 3) as u8,
            DESC_TYPE_CLASS_SPECIFIC_VIDEO,
            subtype,
        ];
        descriptor.extend_from_slice(payload);
        descriptor
    }

    fn frame_descriptor(index: u8, width: u16, height: u16, interval: u32) -> Vec<u8> {
        let mut payload = vec![index, 0];
        payload.extend_from_slice(&width.to_le_bytes());
        payload.extend_from_slice(&height.to_le_bytes());
        payload.extend_from_slice(&1_000_000u32.to_le_bytes());
        payload.extend_from_slice(&10_000_000u32.to_le_bytes());
        payload.extend_from_slice(&interval.to_le_bytes());
        payload.extend_from_slice(&interval.to_le_bytes());
        class_specific_descriptor(DESC_SUBTYPE_FRAME_MJPEG, &payload)
    }

    fn endpoint_descriptor(max_packet_size: u16) -> Vec<u8> {
        let mut descriptor = vec![7, DESC_TYPE_ENDPOINT, 0x81, 0x05];
        descriptor.extend_from_slice(&max_packet_size.to_le_bytes());
        descriptor.push(1);
        descriptor
    }

    fn stream_with_endpoint(max_packet_size: u16) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(class_specific_descriptor(
            DESC_SUBTYPE_FORMAT_MJPEG,
            &[1, 2, 8, 1, 4, 3],
        ));
        bytes.extend(frame_descriptor(1, 640, 480, 333_333));
        bytes.extend(frame_descriptor(2, 320, 240, 666_666));
        bytes.extend(endpoint_descriptor(max_packet_size));
        bytes
    }

    #[test]
    fn parses_streaming_formats_frames_and_endpoint() {
        let stream = UvcStreamInterface::parse(1, 2, &stream_with_endpoint(1024)).unwrap();

        assert_eq!(stream.interface_number(), 1);
        assert_eq!(stream.alternate_setting(), 2);
        assert_eq!(stream.class_specific().len(), 3);
        assert_eq!(stream.formats().len(), 1);
        assert_eq!(stream.formats()[0].format_type(), UvcFormatType::Mjpeg);
        assert_eq!(stream.formats()[0].format_index(), 1);
        assert_eq!(stream.formats()[0].frames().len(), 2);
        assert_eq!(stream.formats()[0].frames()[0].width(), 640);
        assert_eq!(stream.formats()[0].frames()[0].height(), 480);
        assert_eq!(stream.endpoints().len(), 1);
        assert_eq!(stream.endpoint_bandwidth_bytes_per_second(), 1024 * 8000);
    }

    #[test]
    fn selects_alternate_setting_with_highest_endpoint_bandwidth() {
        let low = UvcStreamInterface::parse(1, 0, &stream_with_endpoint(512)).unwrap();
        let high = UvcStreamInterface::parse(1, 1, &stream_with_endpoint(1024)).unwrap();
        let collection = UvcStreamCollection::new(vec![low, high.clone()]);

        assert_eq!(collection.select_alternate_setting(), Some(&high));
    }

    #[test]
    fn rejects_truncated_descriptor_stream() {
        let mut bytes = stream_with_endpoint(1024);
        bytes.truncate(bytes.len() - 1);

        assert!(UvcStreamInterface::parse(1, 2, &bytes).is_err());
    }
}
