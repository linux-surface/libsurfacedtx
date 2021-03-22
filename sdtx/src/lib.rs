use std::convert::TryFrom;
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::path::Path;

pub mod uapi;

pub mod event;
pub use event::{Event, EventStream};


#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("I/O error")]
    IoError { #[from] source: std::io::Error },

    #[error("Kernel API/protocol failure")]
    ProtocolError { #[from] source: ProtocolError },
}

#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("Invalid value for base state: {0:#04x}")]
    InvalidBaseState(u16),

    #[error("Invalid value for device mode: {0:#04x}")]
    InvalidDeviceMode(u16),

    #[error("Invalid value for latch status: {0:#04x}")]
    InvalidLatchStatus(u16),

    #[error("Invalid value for cancel reason: {0:#04x}")]
    InvalidCancelReason(u16),
}

#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeError {
    #[error("Detachment preconditions not fulfilled")]
    NotFeasible,

    #[error("Detach operation timed out")]
    Timeout,

    #[error("Unknown runtime error: {0:#04x}")]
    Unknown(u8),
}

#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareError {
    #[error("Failed to open latch")]
    FailedToOpen,

    #[error("Latch failed to remain open")]
    FailedToRemainOpen,

    #[error("Failed to close latch")]
    FailedToClose,

    #[error("Unknown hardware error: {0:#04x}")]
    Unknown(u8),
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceMode {
    Tablet,
    Laptop,
    Studio,
}

impl std::fmt::Display for DeviceMode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let name = match self {
            DeviceMode::Tablet => "Tablet",
            DeviceMode::Laptop => "Laptop",
            DeviceMode::Studio => "Studio",
        };

        write!(f, "{}", name)
    }
}

impl TryFrom<u16> for DeviceMode {
    type Error = ProtocolError;

    fn try_from(value: u16) -> Result<Self, ProtocolError> {
        match value {
            uapi::SDTX_DEVICE_MODE_TABLET => Ok(DeviceMode::Tablet),
            uapi::SDTX_DEVICE_MODE_LAPTOP => Ok(DeviceMode::Laptop),
            uapi::SDTX_DEVICE_MODE_STUDIO => Ok(DeviceMode::Studio),
            v => Err(ProtocolError::InvalidDeviceMode(v)),
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatchStatus {
    Closed,
    Opened,
    Error(HardwareError),
}

impl std::fmt::Display for LatchStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            LatchStatus::Closed => write!(f, "Closed"),
            LatchStatus::Opened => write!(f, "Opened"),
            LatchStatus::Error(err) => write!(f, "Error: {}", err),
        }
    }
}

impl TryFrom<u16> for LatchStatus {
    type Error = ProtocolError;

    fn try_from(value: u16) -> Result<Self, ProtocolError> {
        use uapi::*;

        match value & uapi::SDTX_CATEGORY_MASK {
            SDTX_CATEGORY_HARDWARE_ERROR => match value {
                SDTX_ERR_FAILED_TO_OPEN        => Ok(Self::Error(HardwareError::FailedToOpen)),
                SDTX_ERR_FAILED_TO_REMAIN_OPEN => Ok(Self::Error(HardwareError::FailedToRemainOpen)),
                SDTX_ERR_FAILED_TO_CLOSE       => Ok(Self::Error(HardwareError::FailedToClose)),
                x                              => Ok(Self::Error(HardwareError::Unknown(x as u8))),
            },
            SDTX_CATEGORY_STATUS => match value {
                SDTX_LATCH_CLOSED              => Ok(Self::Closed),
                SDTX_LATCH_OPENED              => Ok(Self::Opened),
                _ => Err(ProtocolError::InvalidLatchStatus(value)),
            },
            _ => Err(ProtocolError::InvalidLatchStatus(value)),
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseState {
    Detached,
    Attached,
    NotFeasible,
}

impl std::fmt::Display for BaseState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let name = match self {
            BaseState::Detached => "Detached",
            BaseState::Attached => "Attached",
            BaseState::NotFeasible => "NotFeasible",
        };

        write!(f, "{}", name)
    }
}

impl TryFrom<u16> for BaseState {
    type Error = ProtocolError;

    fn try_from(value: u16) -> Result<Self, ProtocolError> {
        match value {
            uapi::SDTX_BASE_DETACHED       => Ok(BaseState::Detached),
            uapi::SDTX_BASE_ATTACHED       => Ok(BaseState::Attached),
            uapi::SDTX_DETACH_NOT_FEASIBLE => Ok(BaseState::NotFeasible),
            v => Err(ProtocolError::InvalidBaseState(v)),
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    Hid,
    Ssh,
    Unknown(u8),
}

impl std::fmt::Display for DeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            DeviceType::Hid        => write!(f, "HID"),
            DeviceType::Ssh        => write!(f, "SSH"),
            DeviceType::Unknown(v) => write!(f, "{:#02x}", v),
        }
    }
}

impl From<u16> for DeviceType {
    fn from(value: u16) -> Self {
        match value & uapi::SDTX_DEVICE_TYPE_MASK {
            uapi::SDTX_DEVICE_TYPE_HID => DeviceType::Hid,
            uapi::SDTX_DEVICE_TYPE_SSH => DeviceType::Ssh,
            v => DeviceType::Unknown((v >> 8) as u8),
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaseInfo {
    pub state: BaseState,
    pub device_type: DeviceType,
    pub id: u8,
}

impl TryFrom<uapi::BaseInfo> for BaseInfo {
    type Error = ProtocolError;

    fn try_from(value: uapi::BaseInfo) -> Result<Self, ProtocolError> {
        let state = BaseState::try_from(value.state)?;
        let device_type = DeviceType::from(value.base_id);
        let id = (value.base_id & 0xff) as u8;

        Ok(BaseInfo { state, device_type, id })
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelReason {
    Runtime(RuntimeError),
    Hardware(HardwareError),
}


#[derive(Debug)]
pub struct Device {
    file: File,
}

impl Device {
    const DEFAULT_DEVICE_FILE_PATH: &'static str = "/dev/surface/dtx";

    pub fn open() -> std::io::Result<Self> {
        Device::open_path(Device::DEFAULT_DEVICE_FILE_PATH)
    }

    pub fn open_path<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        Ok(Device {
            file: File::open(path)?,
        })
    }

    pub fn latch_lock(&self) -> std::io::Result<()> {
        unsafe { uapi::dtx_latch_lock(self.file.as_raw_fd()) }
            .map_err(nix_to_io_err)
            .map(|_| ())
    }

    pub fn latch_unlock(&self) -> std::io::Result<()> {
        unsafe { uapi::dtx_latch_unlock(self.file.as_raw_fd()) }
            .map_err(nix_to_io_err)
            .map(|_| ())
    }

    pub fn latch_request(&self) -> std::io::Result<()> {
        unsafe { uapi::dtx_latch_request(self.file.as_raw_fd()) }
            .map_err(nix_to_io_err)
            .map(|_| ())
    }

    pub fn latch_confirm(&self) -> std::io::Result<()> {
        unsafe { uapi::dtx_latch_confirm(self.file.as_raw_fd()) }
            .map_err(nix_to_io_err)
            .map(|_| ())
    }

    pub fn latch_heartbeat(&self) -> std::io::Result<()> {
        unsafe { uapi::dtx_latch_heartbeat(self.file.as_raw_fd()) }
            .map_err(nix_to_io_err)
            .map(|_| ())
    }

    pub fn latch_cancel(&self) -> std::io::Result<()> {
        unsafe { uapi::dtx_latch_cancel(self.file.as_raw_fd()) }
            .map_err(nix_to_io_err)
            .map(|_| ())
    }

    pub fn get_base_info(&self) -> Result<BaseInfo, Error> {
        let mut info = uapi::BaseInfo {
            state: 0,
            base_id: 0,
        };

        unsafe { uapi::dtx_get_base_info(self.file.as_raw_fd(), &mut info as *mut uapi::BaseInfo) }
            .map_err(nix_to_dtx_err)?;

        Ok(BaseInfo::try_from(info)?)
    }

    pub fn get_device_mode(&self) -> Result<DeviceMode, Error> {
        let mut mode: u16 = 0;

        unsafe { uapi::dtx_get_device_mode(self.file.as_raw_fd(), &mut mode as *mut u16) }
            .map_err(nix_to_dtx_err)?;

        Ok(DeviceMode::try_from(mode)?)
    }

    pub fn get_latch_status(&self) -> Result<LatchStatus, Error> {
        let mut status: u16 = 0;

        unsafe { uapi::dtx_get_latch_status(self.file.as_raw_fd(), &mut status as *mut u16) }
            .map_err(nix_to_dtx_err)?;

        Ok(LatchStatus::try_from(status)?)
    }

    pub fn events(&mut self) -> std::io::Result<EventStream> {
        EventStream::from_device(self)
    }

    fn events_enable(&self) -> std::io::Result<()> {
        unsafe { uapi::dtx_events_enable(self.file.as_raw_fd()) }
            .map_err(nix_to_io_err)
            .map(|_| ())
    }

    fn events_disable(&self) -> std::io::Result<()> {
        unsafe { uapi::dtx_events_disable(self.file.as_raw_fd()) }
            .map_err(nix_to_io_err)
            .map(|_| ())
    }
}


fn nix_to_io_err(err: nix::Error) -> std::io::Error {
    use std::io::{Error, ErrorKind};

    match err {
        nix::Error::Sys(errno)           => Error::from_raw_os_error(errno as i32),
        nix::Error::InvalidPath          => Error::new(ErrorKind::InvalidInput, err),
        nix::Error::InvalidUtf8          => Error::new(ErrorKind::InvalidData, err),
        nix::Error::UnsupportedOperation => Error::new(ErrorKind::Other, err),
    }
}

fn nix_to_dtx_err(err: nix::Error) -> Error {
    nix_to_io_err(err).into()
}
