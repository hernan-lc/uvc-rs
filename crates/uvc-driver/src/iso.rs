use std::{
    mem,
    ptr::{self, NonNull},
    sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering},
    time::{Duration, Instant},
};

use libusb1_sys::{
    constants::{
        LIBUSB_TRANSFER_CANCELLED, LIBUSB_TRANSFER_COMPLETED, LIBUSB_TRANSFER_ERROR,
        LIBUSB_TRANSFER_TIMED_OUT,
    },
    libusb_alloc_transfer, libusb_cancel_transfer, libusb_fill_iso_transfer, libusb_free_transfer,
    libusb_handle_events_timeout_completed, libusb_set_iso_packet_lengths, libusb_submit_transfer,
    libusb_transfer,
};
use uvc_core::{EngineError, EngineResult};

use crate::{CompletedTransfer, RusbUsbDeviceSession, TransferLoop, UsbEndpoint, UsbTransferType};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IsoPacketLayout {
    packet_len: u32,
    packet_count: usize,
    total_len: usize,
}

impl IsoPacketLayout {
    pub fn new(packet_len: u32, packet_count: usize) -> EngineResult<Self> {
        if packet_len == 0 {
            return Err(EngineError::InvalidArgument(
                "ISO packet length must be greater than zero".to_owned(),
            ));
        }

        if packet_count == 0 {
            return Err(EngineError::InvalidArgument(
                "ISO packet count must be greater than zero".to_owned(),
            ));
        }

        let total_len = usize::try_from(packet_len)
            .ok()
            .and_then(|len| len.checked_mul(packet_count))
            .ok_or_else(|| {
                EngineError::InvalidArgument("ISO transfer buffer is too large".to_owned())
            })?;

        Ok(Self {
            packet_len,
            packet_count,
            total_len,
        })
    }

    pub fn packet_len(self) -> u32 {
        self.packet_len
    }

    pub fn packet_count(self) -> usize {
        self.packet_count
    }

    pub fn total_len(self) -> usize {
        self.total_len
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletedIsoTransfer {
    endpoint_address: u8,
    packets: Vec<Vec<u8>>,
    total_len: usize,
}

impl CompletedIsoTransfer {
    pub fn endpoint_address(&self) -> u8 {
        self.endpoint_address
    }

    pub fn packets(&self) -> &[Vec<u8>] {
        &self.packets
    }

    pub fn total_len(&self) -> usize {
        self.total_len
    }

    pub fn into_packets(self) -> Vec<Vec<u8>> {
        self.packets
    }
}

pub struct LibusbIsochronousLoop {
    context: *mut libusb1_sys::libusb_context,
    handle: *mut libusb1_sys::libusb_device_handle,
    endpoint: UsbEndpoint,
    layout: IsoPacketLayout,
    timeout: Duration,
    in_flight: Option<LibusbIsoTransfer>,
    last_packets: Vec<Vec<u8>>,
    last_total_len: usize,
}

impl LibusbIsochronousLoop {
    pub fn new(
        session: &RusbUsbDeviceSession,
        endpoint: UsbEndpoint,
        packet_count: usize,
        timeout: Duration,
    ) -> EngineResult<Self> {
        if endpoint.transfer_type() != UsbTransferType::Isochronous {
            return Err(EngineError::InvalidArgument(format!(
                "endpoint 0x{:02x} is not isochronous",
                endpoint.address()
            )));
        }

        if endpoint.address() & 0x80 == 0 {
            return Err(EngineError::InvalidArgument(format!(
                "endpoint 0x{:02x} is not an IN endpoint",
                endpoint.address()
            )));
        }

        let layout = IsoPacketLayout::new(endpoint.packet_payload_size().into(), packet_count)?;

        Ok(Self {
            context: session.raw_context(),
            handle: session.raw_handle(),
            endpoint,
            layout,
            timeout,
            in_flight: None,
            last_packets: Vec::new(),
            last_total_len: 0,
        })
    }

    pub fn endpoint(&self) -> &UsbEndpoint {
        &self.endpoint
    }

    pub fn layout(&self) -> IsoPacketLayout {
        self.layout
    }

    pub fn last_packets(&self) -> &[Vec<u8>] {
        &self.last_packets
    }

    pub fn last_total_len(&self) -> usize {
        self.last_total_len
    }

    pub fn poll_iso(&mut self) -> EngineResult<Option<CompletedIsoTransfer>> {
        if self.in_flight.is_none() {
            self.submit()?;
        }

        let transfer = self
            .in_flight
            .as_mut()
            .ok_or_else(|| EngineError::Backend("ISO transfer is not in flight".to_owned()))?;

        transfer.wait(self.context, self.timeout)?;
        let completed = transfer.to_completed_iso_transfer(self.endpoint.address())?;
        let total_len = completed.total_len();
        let packets = completed.into_packets();

        self.last_packets = packets;
        self.last_total_len = total_len;
        self.in_flight = None;
        self.submit()?;

        Ok(Some(CompletedIsoTransfer {
            endpoint_address: self.endpoint.address(),
            packets: self.last_packets.clone(),
            total_len: self.last_total_len,
        }))
    }

    fn submit(&mut self) -> EngineResult<()> {
        if self.in_flight.is_some() {
            return Ok(());
        }

        let transfer = LibusbIsoTransfer::submit(
            self.context,
            self.handle,
            self.endpoint.address(),
            self.layout.packet_len(),
            self.layout.packet_count(),
            self.timeout,
        )?;

        self.in_flight = Some(transfer);
        Ok(())
    }
}

impl TransferLoop for LibusbIsochronousLoop {
    fn poll(&mut self) -> EngineResult<Option<CompletedTransfer>> {
        self.poll_iso()?
            .map(|completed| {
                Ok(Some(CompletedTransfer::new(
                    completed.endpoint_address(),
                    completed.total_len(),
                )))
            })
            .unwrap_or(Ok(None))
    }
}

struct LibusbIsoTransfer {
    context: *mut libusb1_sys::libusb_context,
    transfer: NonNull<libusb_transfer>,
    buffer: Vec<u8>,
    state: *mut IsoTransferState,
    layout: IsoPacketLayout,
}

impl LibusbIsoTransfer {
    fn submit(
        context: *mut libusb1_sys::libusb_context,
        handle: *mut libusb1_sys::libusb_device_handle,
        endpoint: u8,
        packet_len: u32,
        packet_count: usize,
        timeout: Duration,
    ) -> EngineResult<Self> {
        let layout = IsoPacketLayout::new(packet_len, packet_count)?;
        let transfer = unsafe { libusb_alloc_transfer(layout.packet_count() as i32) };
        let transfer = NonNull::new(transfer).ok_or_else(|| {
            EngineError::Backend("libusb_alloc_transfer returned null".to_owned())
        })?;
        let mut buffer = vec![0u8; layout.total_len()];
        let state = Box::new(IsoTransferState::new(layout.packet_count()));
        let state_ptr = Box::into_raw(state);

        unsafe {
            libusb_fill_iso_transfer(
                transfer.as_ptr(),
                handle,
                endpoint,
                buffer.as_mut_ptr(),
                layout.total_len() as i32,
                layout.packet_count() as i32,
                iso_transfer_callback,
                state_ptr.cast(),
                duration_to_millis(timeout),
            );
            libusb_set_iso_packet_lengths(transfer.as_ptr(), packet_len);

            let result = libusb_submit_transfer(transfer.as_ptr());
            if result != 0 {
                let _ = Box::from_raw(state_ptr);
                libusb_free_transfer(transfer.as_ptr());
                return Err(libusb_error(result));
            }
        }

        Ok(Self {
            context,
            transfer,
            buffer,
            state: state_ptr,
            layout,
        })
    }

    fn wait(
        &mut self,
        context: *mut libusb1_sys::libusb_context,
        timeout: Duration,
    ) -> EngineResult<()> {
        let deadline = Instant::now() + timeout;

        loop {
            if self.completed() {
                break;
            }

            let remaining = deadline.saturating_duration_since(Instant::now());

            if remaining.is_zero() {
                return Err(EngineError::Timeout);
            }

            let mut tv = duration_to_timeval(remaining.min(Duration::from_millis(100)));
            let result = unsafe {
                libusb_handle_events_timeout_completed(context, &mut tv, ptr::null_mut())
            };

            if result != 0 {
                return Err(libusb_error(result));
            }
        }

        self.status_result()
    }

    fn to_completed_iso_transfer(
        &mut self,
        endpoint_address: u8,
    ) -> EngineResult<CompletedIsoTransfer> {
        let state = unsafe { &*self.state };
        let mut packets = Vec::with_capacity(self.layout.packet_count());

        for packet_index in 0..self.layout.packet_count() {
            let actual_len = state.packet_actual_len(packet_index) as usize;
            let status = state.packet_status(packet_index);

            if status != 0 {
                return Err(libusb_error(status));
            }

            let offset = packet_index * self.layout.packet_len() as usize;
            let end = offset + actual_len;
            packets.push(self.buffer[offset..end].to_vec());
        }

        let total_len = packets.iter().map(Vec::len).sum();

        Ok(CompletedIsoTransfer {
            endpoint_address,
            packets,
            total_len,
        })
    }

    fn completed(&self) -> bool {
        unsafe { (&*self.state).completed.load(Ordering::Acquire) }
    }

    fn status_result(&self) -> EngineResult<()> {
        match unsafe { (&*self.state).status.load(Ordering::Acquire) } {
            LIBUSB_TRANSFER_COMPLETED => Ok(()),
            LIBUSB_TRANSFER_TIMED_OUT => Err(EngineError::Timeout),
            LIBUSB_TRANSFER_CANCELLED => Err(EngineError::Backend(
                "ISO transfer was cancelled".to_owned(),
            )),
            LIBUSB_TRANSFER_ERROR => Err(EngineError::Backend(
                "ISO transfer completed with libusb error".to_owned(),
            )),
            status => Err(EngineError::Backend(format!(
                "ISO transfer completed with unexpected status {status}"
            ))),
        }
    }
}

impl Drop for LibusbIsoTransfer {
    fn drop(&mut self) {
        if !self.completed() {
            unsafe {
                let _ = libusb_cancel_transfer(self.transfer.as_ptr());
            }

            while !self.completed() {
                let mut tv = duration_to_timeval(Duration::from_millis(100));
                let result = unsafe {
                    libusb_handle_events_timeout_completed(self.context, &mut tv, ptr::null_mut())
                };

                if result != 0 {
                    break;
                }
            }
        }

        unsafe {
            let _ = Box::from_raw(self.state);
            libusb_free_transfer(self.transfer.as_ptr());
        }
    }
}

struct IsoTransferState {
    completed: AtomicBool,
    status: AtomicI32,
    actual_length: AtomicI32,
    packet_actual_lengths: Vec<AtomicU32>,
    packet_statuses: Vec<AtomicI32>,
}

impl IsoTransferState {
    fn new(packet_count: usize) -> Self {
        Self {
            completed: AtomicBool::new(false),
            status: AtomicI32::new(LIBUSB_TRANSFER_ERROR),
            actual_length: AtomicI32::new(0),
            packet_actual_lengths: (0..packet_count).map(|_| AtomicU32::new(0)).collect(),
            packet_statuses: (0..packet_count)
                .map(|_| AtomicI32::new(LIBUSB_TRANSFER_ERROR))
                .collect(),
        }
    }

    fn packet_actual_len(&self, index: usize) -> u32 {
        self.packet_actual_lengths[index].load(Ordering::Acquire)
    }

    fn packet_status(&self, index: usize) -> i32 {
        self.packet_statuses[index].load(Ordering::Acquire)
    }
}

extern "system" fn iso_transfer_callback(transfer: *mut libusb_transfer) {
    unsafe {
        let state = &*((*transfer).user_data.cast::<IsoTransferState>());
        state.status.store((*transfer).status, Ordering::Release);
        state
            .actual_length
            .store((*transfer).actual_length, Ordering::Release);

        let packet_count = (*transfer).num_iso_packets as usize;
        let packet_desc = (*transfer).iso_packet_desc.as_ptr();

        for index in 0..packet_count {
            let mut packet = mem::zeroed();
            ptr::copy_nonoverlapping(packet_desc.add(index), &mut packet, 1);
            state.packet_actual_lengths[index].store(packet.actual_length, Ordering::Release);
            state.packet_statuses[index].store(packet.status, Ordering::Release);
        }

        state.completed.store(true, Ordering::Release);
    }
}

fn duration_to_millis(duration: Duration) -> u32 {
    u32::try_from(duration.as_millis()).unwrap_or(u32::MAX)
}

fn duration_to_timeval(duration: Duration) -> libc::timeval {
    let mut value: libc::timeval = unsafe { mem::zeroed() };
    value.tv_sec = duration.as_secs().try_into().unwrap_or(i32::MAX);
    value.tv_usec = duration.subsec_micros().try_into().unwrap_or(i32::MAX);
    value
}

fn libusb_error(code: i32) -> EngineError {
    EngineError::Backend(format!("libusb error {code}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_packet_layout_rejects_invalid_sizes() {
        assert!(IsoPacketLayout::new(0, 4).is_err());
        assert!(IsoPacketLayout::new(1024, 0).is_err());
    }

    #[test]
    fn iso_packet_layout_calculates_total_len() {
        let layout = IsoPacketLayout::new(1024, 4).unwrap();

        assert_eq!(layout.packet_len(), 1024);
        assert_eq!(layout.packet_count(), 4);
        assert_eq!(layout.total_len(), 4096);
    }

    #[test]
    fn iso_packet_layout_rejects_overflow() {
        assert!(IsoPacketLayout::new(u32::MAX, usize::MAX).is_err());
    }
}
