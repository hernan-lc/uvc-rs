use std::time::Duration;

use uvc_core::{EngineError, EngineResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TransferKind {
    Control,
    Isochronous,
    Interrupt,
    Bulk,
}

impl TransferKind {
    pub fn from_endpoint_type(value: u8) -> EngineResult<Self> {
        match value & 0x03 {
            0x00 => Ok(Self::Control),
            0x01 => Ok(Self::Isochronous),
            0x02 => Ok(Self::Interrupt),
            0x03 => Ok(Self::Bulk),
            value => Err(EngineError::InvalidArgument(format!(
                "unknown USB endpoint transfer type {value}"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TransferDirection {
    In,
    Out,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferRequest {
    endpoint_address: u8,
    direction: TransferDirection,
    kind: TransferKind,
    buffer_len: usize,
    timeout: Duration,
}

impl TransferRequest {
    pub fn new(
        endpoint_address: u8,
        direction: TransferDirection,
        kind: TransferKind,
        buffer_len: usize,
        timeout: Duration,
    ) -> EngineResult<Self> {
        if buffer_len == 0 {
            return Err(EngineError::InvalidArgument(
                "transfer buffer length must be greater than zero".to_owned(),
            ));
        }

        Ok(Self {
            endpoint_address,
            direction,
            kind,
            buffer_len,
            timeout,
        })
    }

    pub fn iso_in(
        endpoint_address: u8,
        buffer_len: usize,
        timeout: Duration,
    ) -> EngineResult<Self> {
        Self::new(
            endpoint_address,
            TransferDirection::In,
            TransferKind::Isochronous,
            buffer_len,
            timeout,
        )
    }

    pub fn endpoint_address(&self) -> u8 {
        self.endpoint_address
    }

    pub fn direction(&self) -> TransferDirection {
        self.direction
    }

    pub fn kind(&self) -> TransferKind {
        self.kind
    }

    pub fn buffer_len(&self) -> usize {
        self.buffer_len
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

pub trait TransferLoop {
    fn poll(&mut self) -> EngineResult<Option<CompletedTransfer>>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferBuffer {
    request: TransferRequest,
    data: Vec<u8>,
}

impl TransferBuffer {
    pub fn new(request: TransferRequest) -> Self {
        Self {
            data: vec![0; request.buffer_len()],
            request,
        }
    }

    pub fn request(&self) -> &TransferRequest {
        &self.request
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn into_data(self) -> Vec<u8> {
        self.data
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletedTransfer {
    endpoint_address: u8,
    transferred_len: usize,
}

impl CompletedTransfer {
    pub fn new(endpoint_address: u8, transferred_len: usize) -> Self {
        Self {
            endpoint_address,
            transferred_len,
        }
    }

    pub fn endpoint_address(&self) -> u8 {
        self.endpoint_address
    }

    pub fn transferred_len(&self) -> usize {
        self.transferred_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn transfer_request_rejects_empty_buffer() {
        assert!(TransferRequest::iso_in(0x81, 0, Duration::from_millis(10)).is_err());
    }

    #[test]
    fn transfer_request_tracks_iso_in_parameters() {
        let request = TransferRequest::iso_in(0x81, 1024, Duration::from_millis(10)).unwrap();

        assert_eq!(request.endpoint_address(), 0x81);
        assert_eq!(request.direction(), TransferDirection::In);
        assert_eq!(request.kind(), TransferKind::Isochronous);
        assert_eq!(request.buffer_len(), 1024);
        assert_eq!(request.timeout(), Duration::from_millis(10));
    }

    #[test]
    fn transfer_buffer_allocates_requested_size() {
        let request = TransferRequest::iso_in(0x81, 1024, Duration::from_millis(10)).unwrap();
        let mut buffer = TransferBuffer::new(request.clone());

        assert_eq!(buffer.request(), &request);
        assert_eq!(buffer.data().len(), 1024);
        assert_eq!(buffer.data_mut().len(), 1024);
    }
}
