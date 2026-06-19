use uvc_core::{CameraId, EngineError, EngineResult, Frame, FrameBuffer, FrameFormat, FrameSink};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UvcPayloadHeader {
    header_len: usize,
    frame_id: bool,
    end_of_frame: bool,
    end_of_header: u8,
}

impl UvcPayloadHeader {
    pub fn parse(packet: &[u8]) -> EngineResult<Self> {
        if packet.len() < 2 {
            return Err(EngineError::InvalidArgument(
                "UVC payload packet is too short for a payload header".to_owned(),
            ));
        }

        let header_len = usize::from(packet[0]);
        let header_info = packet[1];

        if header_len < 2 || header_len > packet.len() {
            return Err(EngineError::InvalidArgument(format!(
                "invalid UVC payload header length {header_len} for packet length {}",
                packet.len()
            )));
        }

        Ok(Self {
            header_len,
            frame_id: header_info & 0x01 != 0,
            end_of_frame: header_info & 0x02 != 0,
            end_of_header: (header_info >> 6) & 0x03,
        })
    }

    pub fn header_len(self) -> usize {
        self.header_len
    }

    pub fn frame_id(self) -> bool {
        self.frame_id
    }

    pub fn end_of_frame(self) -> bool {
        self.end_of_frame
    }

    pub fn end_of_header(self) -> u8 {
        self.end_of_header
    }

    pub fn has_header(self) -> bool {
        self.end_of_header != 0x03
    }

    pub fn data_start(self) -> usize {
        if self.has_header() {
            self.header_len
        } else {
            0
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UvcPacketAssembler {
    frame_id: Option<bool>,
    buffer: Vec<u8>,
}

impl UvcPacketAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_packet(&mut self, packet: &[u8]) -> EngineResult<Option<Vec<u8>>> {
        if packet.is_empty() {
            return Ok(None);
        }

        let header = UvcPayloadHeader::parse(packet).ok();
        let frame_id = header.map(UvcPayloadHeader::frame_id);
        let data_start = header.map(UvcPayloadHeader::data_start).unwrap_or(0);
        let end_of_frame = header.is_some_and(UvcPayloadHeader::end_of_frame);

        if let (Some(current), Some(next)) = (self.frame_id, frame_id) {
            if current != next && !self.buffer.is_empty() {
                return Err(EngineError::Backend(
                    "UVC frame ID changed before end-of-frame".to_owned(),
                ));
            }
        }

        if self.buffer.is_empty() {
            self.frame_id = frame_id;
        }

        self.buffer.extend_from_slice(&packet[data_start..]);

        if end_of_frame {
            let frame = std::mem::take(&mut self.buffer);
            self.frame_id = None;
            Ok(Some(frame))
        } else {
            Ok(None)
        }
    }

    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_assembling(&self) -> bool {
        !self.buffer.is_empty()
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.frame_id = None;
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MjpegFrameAssembler {
    inner: UvcPacketAssembler,
}

impl MjpegFrameAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_packet(&mut self, packet: &[u8]) -> EngineResult<Option<Vec<u8>>> {
        let Some(frame) = self.inner.push_packet(packet)? else {
            return Ok(None);
        };

        if is_mjpeg_frame(&frame) {
            Ok(Some(frame))
        } else {
            Err(EngineError::Backend(
                "assembled frame is not a valid MJPEG frame".to_owned(),
            ))
        }
    }

    pub fn buffer_len(&self) -> usize {
        self.inner.buffer_len()
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

pub fn is_mjpeg_frame(frame: &[u8]) -> bool {
    frame.starts_with(&[0xff, 0xd8]) && frame.ends_with(&[0xff, 0xd9])
}

#[derive(Clone, Debug)]
pub struct MjpegFrameSinkAdapter<S> {
    sink: S,
    assembler: MjpegFrameAssembler,
    camera_id: CameraId,
    format: FrameFormat,
    sequence: u64,
}

impl<S> MjpegFrameSinkAdapter<S>
where
    S: FrameSink,
{
    pub fn new(sink: S, camera_id: CameraId, format: FrameFormat) -> Self {
        Self {
            sink,
            assembler: MjpegFrameAssembler::new(),
            camera_id,
            format,
            sequence: 0,
        }
    }

    pub fn push_packet(&mut self, packet: &[u8]) -> EngineResult<bool> {
        let Some(frame_data) = self.assembler.push_packet(packet)? else {
            return Ok(false);
        };

        let frame = self.make_frame(frame_data);
        self.sink.push(frame)?;
        Ok(true)
    }

    pub fn buffer_len(&self) -> usize {
        self.assembler.buffer_len()
    }

    pub fn clear(&mut self) {
        self.assembler.clear();
    }

    fn make_frame(&mut self, data: Vec<u8>) -> Frame {
        let sequence = self.sequence;
        self.sequence += 1;

        Frame::new(
            self.camera_id.clone(),
            sequence,
            FrameBuffer::new(self.format, data).expect("MJPEG frame length is variable"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct VecFrameSink {
        frames: Vec<Frame>,
    }

    impl FrameSink for VecFrameSink {
        fn push(&mut self, frame: Frame) -> EngineResult<()> {
            self.frames.push(frame);
            Ok(())
        }
    }

    fn packet(frame_id: bool, end_of_frame: bool, data: &[u8]) -> Vec<u8> {
        let mut header_info = 0;
        if frame_id {
            header_info |= 0x01;
        }
        if end_of_frame {
            header_info |= 0x02;
        }

        let mut packet = vec![2, header_info];
        packet.extend_from_slice(data);
        packet
    }

    #[test]
    fn parses_uvc_payload_header() {
        let header = UvcPayloadHeader::parse(&packet(false, false, &[1, 2])).unwrap();

        assert_eq!(header.header_len(), 2);
        assert!(!header.frame_id());
        assert!(!header.end_of_frame());
        assert_eq!(header.data_start(), 2);
    }

    #[test]
    fn assembles_uvc_packets_until_eof() {
        let mut assembler = UvcPacketAssembler::new();

        assert_eq!(
            assembler
                .push_packet(&packet(false, false, &[1, 2]))
                .unwrap(),
            None
        );
        assert_eq!(assembler.buffer_len(), 2);
        assert_eq!(
            assembler
                .push_packet(&packet(false, true, &[3, 4]))
                .unwrap(),
            Some(vec![1, 2, 3, 4])
        );
        assert!(!assembler.is_assembling());
    }

    #[test]
    fn rejects_frame_id_change_before_eof() {
        let mut assembler = UvcPacketAssembler::new();
        assembler.push_packet(&packet(false, false, &[1])).unwrap();

        assert!(assembler.push_packet(&packet(true, false, &[2])).is_err());
    }

    #[test]
    fn validates_mjpeg_frame_boundaries() {
        assert!(is_mjpeg_frame(&[0xff, 0xd8, 0xaa, 0xff, 0xd9]));
        assert!(!is_mjpeg_frame(&[0xff, 0xd8, 0xaa]));
    }

    #[test]
    fn mjpeg_frame_sink_adapter_emits_frame() {
        let sink = VecFrameSink::default();
        let mut adapter = MjpegFrameSinkAdapter::new(
            sink,
            CameraId::new("cam-1").unwrap(),
            FrameFormat::mjpeg(640, 480, 30).unwrap(),
        );
        let frame_data = vec![0xff, 0xd8, 0xaa, 0xff, 0xd9];

        assert!(
            !adapter
                .push_packet(&packet(false, false, &frame_data[..3]))
                .unwrap()
        );
        assert!(
            adapter
                .push_packet(&packet(false, true, &frame_data[3..]))
                .unwrap()
        );
        assert_eq!(adapter.buffer_len(), 0);

        let frames = &adapter.sink.frames;
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].camera_id().as_str(), "cam-1");
        assert_eq!(frames[0].sequence(), 0);
        assert_eq!(frames[0].buffer().as_slice(), frame_data.as_slice());
    }

    #[test]
    fn mjpeg_assembler_rejects_invalid_frame() {
        let mut assembler = MjpegFrameAssembler::new();

        assert!(
            assembler
                .push_packet(&packet(false, true, &[1, 2]))
                .is_err()
        );
    }
}
