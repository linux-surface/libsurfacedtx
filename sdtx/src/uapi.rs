#![allow(clippy::identity_op)]

use nix::{ioctl_none, ioctl_read};


// status/error categories
pub const SDTX_CATEGORY_STATUS: u16 = 0x0000;
pub const SDTX_CATEGORY_RUNTIME_ERROR: u16 = 0x1000;
pub const SDTX_CATEGORY_HARDWARE_ERROR: u16 = 0x2000;

pub const SDTX_CATEGORY_MASK: u16 = 0xf000;

// latch status values
pub const SDTX_LATCH_CLOSED: u16 = SDTX_CATEGORY_STATUS | 0x00;
pub const SDTX_LATCH_OPENED: u16 = SDTX_CATEGORY_STATUS | 0x01;

// base status values
pub const SDTX_BASE_DETACHED: u16 = SDTX_CATEGORY_STATUS | 0x00;
pub const SDTX_BASE_ATTACHED: u16 = SDTX_CATEGORY_STATUS | 0x01;

// runtime errors (non-critical)
pub const SDTX_DETACH_NOT_FEASIBLE: u16 = SDTX_CATEGORY_RUNTIME_ERROR | 0x01;
pub const SDTX_DETACH_TIMEOUT: u16 = SDTX_CATEGORY_RUNTIME_ERROR | 0x02;

// hardware errors (critical)
pub const SDTX_ERR_FAILED_TO_OPEN: u16 = SDTX_CATEGORY_HARDWARE_ERROR | 0x01;
pub const SDTX_ERR_FAILED_TO_REMAIN_OPEN: u16 = SDTX_CATEGORY_HARDWARE_ERROR | 0x02;
pub const SDTX_ERR_FAILED_TO_CLOSE: u16 = SDTX_CATEGORY_HARDWARE_ERROR | 0x03;

// base types
pub const SDTX_DEVICE_TYPE_HID: u16 = 0x0100;
pub const SDTX_DEVICE_TYPE_SSH: u16 = 0x0200;

pub const SDTX_DEVICE_TYPE_MASK: u16 = 0x0f00;

// device mode
pub const SDTX_DEVICE_MODE_TABLET: u16 = 0x00;
pub const SDTX_DEVICE_MODE_LAPTOP: u16 = 0x01;
pub const SDTX_DEVICE_MODE_STUDIO: u16 = 0x02;

// event code
pub const SDTX_EVENT_REQUEST: u16 = 1;
pub const SDTX_EVENT_CANCEL: u16 = 2;
pub const SDTX_EVENT_BASE_CONNECTION: u16 = 3;
pub const SDTX_EVENT_LATCH_STATUS: u16 = 4;
pub const SDTX_EVENT_DEVICE_MODE: u16 = 5;

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct EventHeader {
    pub length: u16,
    pub code: u16,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct BaseInfo {
    pub state: u16,
    pub base_id: u16,
}

ioctl_none!(dtx_events_enable, 0xa5, 0x21);
ioctl_none!(dtx_events_disable, 0xa5, 0x22);

ioctl_none!(dtx_latch_lock, 0xa5, 0x23);
ioctl_none!(dtx_latch_unlock, 0xa5, 0x24);

ioctl_none!(dtx_latch_request, 0xa5, 0x25);
ioctl_none!(dtx_latch_confirm, 0xa5, 0x26);
ioctl_none!(dtx_latch_heartbeat, 0xa5, 0x27);
ioctl_none!(dtx_latch_cancel, 0xa5, 0x28);

ioctl_read!(dtx_get_base_info, 0xa5, 0x29, BaseInfo);
ioctl_read!(dtx_get_device_mode, 0xa5, 0x2a, u16);
ioctl_read!(dtx_get_latch_status, 0xa5, 0x2b, u16);
