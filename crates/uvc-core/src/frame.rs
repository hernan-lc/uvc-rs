use std::{
    sync::mpsc::{
        Receiver, RecvError, RecvTimeoutError, SendError, SyncSender, TryRecvError, TrySendError,
        sync_channel,
    },
    time::{Duration, Instant},
};

use crate::{CameraId, EngineError, EngineResult, FrameFormat};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameBuffer {
    format: FrameFormat,
    data: Vec<u8>,
}

impl FrameBuffer {
    pub fn new(format: FrameFormat, data: Vec<u8>) -> EngineResult<Self> {
        format.validate_frame_len(data.len())?;

        Ok(Self { format, data })
    }

    pub fn zeros(format: FrameFormat) -> EngineResult<Self> {
        let len = format.expected_len().ok_or_else(|| {
            EngineError::InvalidFrameFormat(format!(
                "cannot allocate zeros for compressed format `{}`",
                format
            ))
        })?;

        Self::new(format, vec![0; len])
    }

    pub fn format(&self) -> FrameFormat {
        self.format
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    camera_id: CameraId,
    sequence: u64,
    buffer: FrameBuffer,
    created_at: Instant,
}

impl Frame {
    pub fn new(camera_id: CameraId, sequence: u64, buffer: FrameBuffer) -> Self {
        Self {
            camera_id,
            sequence,
            buffer,
            created_at: Instant::now(),
        }
    }

    pub fn camera_id(&self) -> &CameraId {
        &self.camera_id
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn buffer(&self) -> &FrameBuffer {
        &self.buffer
    }

    pub fn into_buffer(self) -> FrameBuffer {
        self.buffer
    }

    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }
}

pub trait FrameSink {
    fn push(&mut self, frame: Frame) -> EngineResult<()>;
}

#[derive(Clone, Debug)]
pub struct FrameSender {
    inner: SyncSender<Frame>,
}

impl FrameSender {
    pub fn send(&self, frame: Frame) -> EngineResult<()> {
        self.inner
            .send(frame)
            .map_err(|SendError(_)| EngineError::SinkClosed)
    }

    pub fn try_send(&self, frame: Frame) -> EngineResult<()> {
        self.inner.try_send(frame).map_err(|error| match error {
            TrySendError::Full(_) | TrySendError::Disconnected(_) => EngineError::SinkClosed,
        })
    }
}

impl FrameSink for FrameSender {
    fn push(&mut self, frame: Frame) -> EngineResult<()> {
        self.send(frame)
    }
}

#[derive(Debug)]
pub struct FrameReceiver {
    inner: Receiver<Frame>,
}

impl FrameReceiver {
    pub fn recv(&self) -> EngineResult<Frame> {
        self.inner
            .recv()
            .map_err(|RecvError| EngineError::SinkClosed)
    }

    pub fn recv_timeout(&self, timeout: Duration) -> EngineResult<Frame> {
        self.inner
            .recv_timeout(timeout)
            .map_err(|error| match error {
                RecvTimeoutError::Timeout => EngineError::Timeout,
                RecvTimeoutError::Disconnected => EngineError::SinkClosed,
            })
    }

    pub fn try_recv(&self) -> EngineResult<Option<Frame>> {
        match self.inner.try_recv() {
            Ok(frame) => Ok(Some(frame)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(EngineError::SinkClosed),
        }
    }
}

pub fn frame_channel(capacity: usize) -> (FrameSender, FrameReceiver) {
    let (sender, receiver) = sync_channel(capacity);

    (
        FrameSender { inner: sender },
        FrameReceiver { inner: receiver },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_frame() -> Frame {
        Frame::new(
            CameraId::new("cam-1").unwrap(),
            1,
            FrameBuffer::new(FrameFormat::rgba(2, 2, 30).unwrap(), vec![0; 16]).unwrap(),
        )
    }

    #[test]
    fn frame_sender_reports_closed_sink() {
        let (sender, receiver) = frame_channel(1);
        drop(receiver);

        assert!(sender.send(test_frame()).is_err());
    }

    #[test]
    fn frame_receiver_reports_timeout() {
        let (_sender, receiver) = frame_channel(1);

        assert!(matches!(
            receiver.recv_timeout(Duration::from_millis(1)),
            Err(EngineError::Timeout)
        ));
    }
}
