use smallvec::SmallVec;
use std::{
    convert::TryInto,
    fs::File,
    io::{BufReader, Read},
};

use crate::{uapi, Device, DeviceType, HardwareError, ProtocolError, RuntimeError};

#[derive(Debug, Clone)]
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



#[derive(Debug, Clone, Copy)]
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

// TODO: switch to TryFrom
impl TryInto<super::CancelReason> for CancelReason {
    type Error = ProtocolError;

    fn try_into(self) -> Result<super::CancelReason, ProtocolError> {
        match self {
            Self::Runtime(err)  => Ok(super::CancelReason::Runtime(err)),
            Self::Hardware(err) => Ok(super::CancelReason::Hardware(err)),
            Self::Unknown(err)  => Err(ProtocolError::InvalidCancelReason(err)),
        }
    }
}

#[derive(Debug, Clone, Copy)]
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

// TODO: switch to TryFrom
impl TryInto<super::BaseState> for BaseState {
    type Error = ProtocolError;

    fn try_into(self) -> Result<super::BaseState, ProtocolError> {
        match self {
            Self::Detached     => Ok(super::BaseState::Detached),
            Self::Attached     => Ok(super::BaseState::Attached),
            Self::NotFeasible  => Ok(super::BaseState::NotFeasible),
            Self::Unknown(err) => Err(ProtocolError::InvalidBaseState(err)),
        }
    }
}

#[derive(Debug, Clone, Copy)]
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

// TODO: switch to TryFrom
impl TryInto<super::LatchStatus> for LatchStatus {
    type Error = ProtocolError;

    fn try_into(self) -> Result<super::LatchStatus, ProtocolError> {
        match self {
            Self::Closed       => Ok(super::LatchStatus::Closed),
            Self::Opened       => Ok(super::LatchStatus::Opened),
            Self::Error(err)   => Ok(super::LatchStatus::Error(err)),
            Self::Unknown(err) => Err(ProtocolError::InvalidLatchStatus(err)),
        }
    }
}

#[derive(Debug, Clone, Copy)]
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

// TODO: switch to TryFrom
impl TryInto<super::DeviceMode> for DeviceMode {
    type Error = ProtocolError;

    fn try_into(self) -> Result<super::DeviceMode, ProtocolError> {
        match self {
            Self::Tablet       => Ok(super::DeviceMode::Tablet),
            Self::Laptop       => Ok(super::DeviceMode::Laptop),
            Self::Studio       => Ok(super::DeviceMode::Studio),
            Self::Unknown(err) => Err(ProtocolError::InvalidDeviceMode(err)),
        }
    }
}

#[derive(Debug)]
pub struct EventStream<'a> {
    device: &'a mut Device,
    reader: BufReader<File>,
}

impl<'a> EventStream<'a> {
    pub(crate) fn from_device(device: &'a mut Device) -> std::io::Result<Self> {
        let file = device.file.try_clone().unwrap();

        device.events_enable()?;

        Ok(EventStream {
            device,
            reader: BufReader::new(file),
        })
    }
}

impl<'a> Drop for EventStream<'a> {
    fn drop(&mut self) {
        let _ = self.device.events_disable();
    }
}

impl<'a> EventStream<'a> {
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

impl<'a> Iterator for EventStream<'a> {
    type Item = std::io::Result<Event>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.read_next_blocking())
    }
}
