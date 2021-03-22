use std::convert::{TryFrom, TryInto};
use std::io::{BufReader, Read};
use std::os::unix::io::AsRawFd;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{AsyncRead, AsyncReadExt, Stream};
use smallvec::SmallVec;

use crate::uapi;
use crate::{Device, DeviceType, HardwareError, ProtocolError, RuntimeError};


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Request,

    Cancel {
        reason: CancelReason,
    },

    BaseConnection {
        state: BaseState,
        device_type: DeviceType,
        id: u8,
    },

    LatchStatus {
        status: LatchStatus,
    },

    DeviceMode {
        mode: DeviceMode,
    },

    Unknown {
        code: u16,
        data: Vec<u8>,
    },
}

impl Event {
    pub fn from_data(code: u16, data: &[u8]) -> Self {
        match code {
            uapi::SDTX_EVENT_REQUEST => {
                if !data.is_empty() {
                    return Event::Unknown { code, data: data.into() };
                }

                Event::Request
            }

            uapi::SDTX_EVENT_CANCEL => {
                if data.len() != std::mem::size_of::<u16>() {
                    return Event::Unknown { code, data: data.into() };
                }

                let reason = &data[0..std::mem::size_of::<u16>()];
                let reason = u16::from_ne_bytes(reason.try_into().unwrap());
                let reason = CancelReason::from(reason);

                Event::Cancel { reason }
            }

            uapi::SDTX_EVENT_BASE_CONNECTION => {
                if data.len() != 2 * std::mem::size_of::<u16>() {
                    return Event::Unknown { code, data: data.into() };
                }

                let state = &data[0..std::mem::size_of::<u16>()];
                let state = u16::from_ne_bytes(state.try_into().unwrap());
                let state = BaseState::from(state);

                let base = &data[std::mem::size_of::<u16>()..2 * std::mem::size_of::<u16>()];
                let base = u16::from_ne_bytes(base.try_into().unwrap());

                let device_type = DeviceType::from(base);
                let id = (base & 0xff) as u8;

                Event::BaseConnection { state, device_type, id }
            }

            uapi::SDTX_EVENT_LATCH_STATUS => {
                if data.len() != std::mem::size_of::<u16>() {
                    return Event::Unknown { code, data: data.into() };
                }

                let status = &data[0..std::mem::size_of::<u16>()];
                let status = u16::from_ne_bytes(status.try_into().unwrap());
                let status = LatchStatus::from(status);

                Event::LatchStatus { status }
            }

            uapi::SDTX_EVENT_DEVICE_MODE => {
                if data.len() != std::mem::size_of::<u16>() {
                    return Event::Unknown { code, data: data.into() };
                }

                let mode = &data[0..std::mem::size_of::<u16>()];
                let mode = u16::from_ne_bytes(mode.try_into().unwrap());
                let mode = DeviceMode::from(mode);

                Event::DeviceMode { mode }
            }

            code => Event::Unknown { code, data: data.into() },
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelReason {
    Runtime(RuntimeError),
    Hardware(HardwareError),
    Unknown(u16),
}

impl From<u16> for CancelReason {
    fn from(value: u16) -> Self {
        use uapi::*;

        match value & uapi::SDTX_CATEGORY_MASK {
            SDTX_CATEGORY_RUNTIME_ERROR => match value {
                SDTX_DETACH_NOT_FEASIBLE       => Self::Runtime(RuntimeError::NotFeasible),
                SDTX_DETACH_TIMEOUT            => Self::Runtime(RuntimeError::Timeout),
                x                              => Self::Runtime(RuntimeError::Unknown(x as u8)),
            },
            SDTX_CATEGORY_HARDWARE_ERROR => match value {
                SDTX_ERR_FAILED_TO_OPEN        => Self::Hardware(HardwareError::FailedToOpen),
                SDTX_ERR_FAILED_TO_REMAIN_OPEN => Self::Hardware(HardwareError::FailedToRemainOpen),
                SDTX_ERR_FAILED_TO_CLOSE       => Self::Hardware(HardwareError::FailedToClose),
                x                              => Self::Hardware(HardwareError::Unknown(x as u8)),
            },
            x => Self::Unknown(x),
        }
    }
}

impl TryFrom<CancelReason> for super::CancelReason {
    type Error = ProtocolError;

    fn try_from(value: CancelReason) -> Result<Self, ProtocolError> {
        match value {
            CancelReason::Runtime(err)  => Ok(Self::Runtime(err)),
            CancelReason::Hardware(err) => Ok(Self::Hardware(err)),
            CancelReason::Unknown(err)  => Err(ProtocolError::InvalidCancelReason(err)),
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseState {
    Detached,
    Attached,
    NotFeasible,
    Unknown(u16),
}

impl From<u16> for BaseState {
    fn from(value: u16) -> Self {
        match value {
            uapi::SDTX_BASE_DETACHED       => Self::Detached,
            uapi::SDTX_BASE_ATTACHED       => Self::Attached,
            uapi::SDTX_DETACH_NOT_FEASIBLE => Self::NotFeasible,
            x => Self::Unknown(x),
        }
    }
}

impl TryFrom<BaseState> for super::BaseState {
    type Error = ProtocolError;

    fn try_from(value: BaseState) -> Result<super::BaseState, ProtocolError> {
        match value {
            BaseState::Detached     => Ok(Self::Detached),
            BaseState::Attached     => Ok(Self::Attached),
            BaseState::NotFeasible  => Ok(Self::NotFeasible),
            BaseState::Unknown(err) => Err(ProtocolError::InvalidBaseState(err)),
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatchStatus {
    Closed,
    Opened,
    Error(HardwareError),
    Unknown(u16),
}

impl From<u16> for LatchStatus {
    fn from(value: u16) -> Self {
        use uapi::*;

        match value & uapi::SDTX_CATEGORY_MASK {
            SDTX_CATEGORY_HARDWARE_ERROR => match value {
                SDTX_ERR_FAILED_TO_OPEN        => Self::Error(HardwareError::FailedToOpen),
                SDTX_ERR_FAILED_TO_REMAIN_OPEN => Self::Error(HardwareError::FailedToRemainOpen),
                SDTX_ERR_FAILED_TO_CLOSE       => Self::Error(HardwareError::FailedToClose),
                x                              => Self::Error(HardwareError::Unknown(x as u8)),
            },
            SDTX_CATEGORY_STATUS => match value {
                SDTX_LATCH_CLOSED => Self::Closed,
                SDTX_LATCH_OPENED => Self::Opened,
                x => Self::Unknown(x),
            },
            x => Self::Unknown(x),
        }
    }
}

impl TryFrom<LatchStatus> for super::LatchStatus {
    type Error = ProtocolError;

    fn try_from(value: LatchStatus) -> Result<super::LatchStatus, ProtocolError> {
        match value {
            LatchStatus::Closed       => Ok(Self::Closed),
            LatchStatus::Opened       => Ok(Self::Opened),
            LatchStatus::Error(err)   => Ok(Self::Error(err)),
            LatchStatus::Unknown(err) => Err(ProtocolError::InvalidLatchStatus(err)),
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceMode {
    Tablet,
    Laptop,
    Studio,
    Unknown(u16),
}

impl From<u16> for DeviceMode {
    fn from(value: u16) -> Self {
        match value {
            uapi::SDTX_DEVICE_MODE_TABLET => Self::Tablet,
            uapi::SDTX_DEVICE_MODE_LAPTOP => Self::Laptop,
            uapi::SDTX_DEVICE_MODE_STUDIO => Self::Studio,
            x => Self::Unknown(x),
        }
    }
}

impl TryFrom<DeviceMode> for super::DeviceMode {
    type Error = ProtocolError;

    fn try_from(value: DeviceMode) -> Result<super::DeviceMode, ProtocolError> {
        match value {
            DeviceMode::Tablet       => Ok(Self::Tablet),
            DeviceMode::Laptop       => Ok(Self::Laptop),
            DeviceMode::Studio       => Ok(Self::Studio),
            DeviceMode::Unknown(err) => Err(ProtocolError::InvalidDeviceMode(err)),
        }
    }
}


#[derive(Debug)]
pub struct EventStream<'a, F: AsRawFd> {
    reader: BufReader<&'a mut F>,
}

impl<'a, F: AsRawFd + Read> EventStream<'a, F> {
    pub(crate) fn from_device(device: &'a mut Device<F>) -> std::io::Result<Self> {
        device.events_enable()?;

        let reader = BufReader::with_capacity(128, device.file_mut());

        Ok(EventStream { reader })
    }
}

impl<'a, F: AsRawFd> Drop for EventStream<'a, F> {
    fn drop(&mut self) {
        let _ = unsafe { uapi::dtx_events_disable(self.reader.get_ref().as_raw_fd()) };
    }
}

impl<'a, F: AsRawFd + Read> EventStream<'a, F> {
    pub fn read_next_blocking(&mut self) -> std::io::Result<Event> {
        let mut buf_hdr = [0; std::mem::size_of::<uapi::EventHeader>()];
        let mut buf_data = SmallVec::<[u8; 32]>::new();

        self.reader.read_exact(&mut buf_hdr)?;

        let hdr: uapi::EventHeader = unsafe { std::mem::transmute_copy(&buf_hdr) };

        buf_data.resize(hdr.length as usize, 0);
        self.reader.read_exact(&mut buf_data)?;

        Ok(Event::from_data(hdr.code, &buf_data))
    }
}

impl<'a, F: AsRawFd + Read> Iterator for EventStream<'a, F> {
    type Item = std::io::Result<Event>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.read_next_blocking())
    }
}


#[derive(Debug)]
pub struct AsyncEventStream<'a, F: AsRawFd + AsyncRead + Unpin> {
    file: &'a mut F,
    buffer: Vec<u8>,
    offset: usize,
}

impl<'a, F: AsRawFd + AsyncRead + Unpin> AsyncEventStream<'a, F> {
    pub(crate) fn from_device(device: &'a mut Device<F>) -> std::io::Result<Self> {
        device.events_enable()?;

        Ok(AsyncEventStream { file: device.file_mut(), buffer: vec![0; 128], offset: 0 })
    }
}

impl<'a, F: AsRawFd + AsyncRead + Unpin> Drop for AsyncEventStream<'a, F> {
    fn drop(&mut self) {
        let _ = unsafe { uapi::dtx_events_disable(self.file.as_raw_fd()) };
    }
}

impl<'a, F: AsRawFd + AsyncRead + Unpin> AsyncEventStream<'a, F> {
    pub async fn read_next(&mut self) -> std::io::Result<Event> {
        const HEADER_LEN: usize = std::mem::size_of::<uapi::EventHeader>();

        while self.offset < HEADER_LEN {
            self.offset += self.file.read(&mut self.buffer[self.offset..]).await?;
        }

        let data_hdr = &self.buffer[..HEADER_LEN];
        let data_hdr: [u8; HEADER_LEN] = data_hdr.try_into().unwrap();
        let hdr: uapi::EventHeader = unsafe { std::mem::transmute_copy(&data_hdr) };

        let event_len = HEADER_LEN+ hdr.length as usize;
        self.buffer.resize(event_len, 0);

        while self.offset < event_len {
            self.offset += self.file.read(&mut self.buffer[self.offset..]).await?;
        }

        let event = Event::from_data(hdr.code, &self.buffer[HEADER_LEN..event_len]);
        self.offset = 0;

        Ok(event)
    }
}

impl<'a, F: AsRawFd + AsyncRead + Unpin> Stream for AsyncEventStream<'a, F> {
    type Item = std::io::Result<Event>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        const HEADER_LEN: usize = std::mem::size_of::<uapi::EventHeader>();

        let s = Pin::into_inner(self);

        if s.offset < HEADER_LEN {
            s.offset += futures::ready!(Pin::new(&mut s.file)
                .poll_read(cx, &mut s.buffer[s.offset..]))?;
        }

        if s.offset < HEADER_LEN {
            return Poll::Pending;
        }

        let data_hdr = &s.buffer[..HEADER_LEN];
        let data_hdr: [u8; HEADER_LEN] = data_hdr.try_into().unwrap();
        let hdr: uapi::EventHeader = unsafe { std::mem::transmute_copy(&data_hdr) };

        let event_len = HEADER_LEN+ hdr.length as usize;

        if s.offset < event_len {
            s.buffer.resize(event_len, 0);

            s.offset += futures::ready!(Pin::new(&mut s.file)
                .poll_read(cx, &mut s.buffer[s.offset..]))?;
        }

        if s.offset < event_len {
            return Poll::Pending;
        }

        let event = Event::from_data(hdr.code, &s.buffer[HEADER_LEN..event_len]);
        s.offset = 0;

        Poll::Ready(Some(Ok(event)))
    }
}
