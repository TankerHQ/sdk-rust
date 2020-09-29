use crate::ctanker::CDevice;
use crate::VerificationMethod;
use num_enum::{TryFromPrimitive, UnsafeFromPrimitive};
use std::ffi::{CStr, CString};
use std::fmt::{Display, Formatter};

/// Options used by the [Core](struct.Core.html) struct
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Options {
    pub(crate) url: Option<CString>,
    pub(crate) app_id: CString,
    pub(crate) writable_path: CString,
}

impl Options {
    /// # Arguments
    /// * `app_id` - Your Tanker App ID
    /// * `writable_path` - A writable folder. Tanker will use this folder to
    ///    store persistent data about user sessions on the current device.
    pub fn new(app_id: String, writable_path: String) -> Self {
        Self {
            url: None,
            app_id: CString::new(app_id).unwrap(),
            writable_path: CString::new(writable_path).unwrap(),
        }
    }

    pub fn with_url(mut self, url: String) -> Self {
        self.url = Some(CString::new(url).unwrap());
        self
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, UnsafeFromPrimitive)]
#[non_exhaustive]
#[repr(u32)]
pub enum Status {
    Stopped = 0,
    Ready = 1,
    IdentityRegistrationNeeded = 2,
    IdentityVerificationNeeded = 3,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, TryFromPrimitive)]
#[non_exhaustive]
#[repr(u32)]
pub enum LogRecordLevel {
    Debug = 1,
    Info = 2,
    Warning = 3,
    Error = 4,
}

#[derive(Debug, Clone)]
pub struct LogRecord {
    pub category: String,
    pub level: LogRecordLevel,
    pub file: String,
    pub line: u32,
    pub message: String,
}

impl Display for LogRecordLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match *self {
            LogRecordLevel::Debug => f.write_str("Debug"),
            LogRecordLevel::Info => f.write_str("Info"),
            LogRecordLevel::Warning => f.write_str("Warning"),
            LogRecordLevel::Error => f.write_str("Error"),
        }
    }
}

/// Describes a Tanker device
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Device {
    /// The device ID of the device
    pub id: String,
    /// A device that is revoked cannot be used anymore
    pub revoked: bool,
}

impl From<&CDevice> for Device {
    fn from(elem: &CDevice) -> Self {
        // SAFETY: If CDevice is valid, the device ID is valid UTF-8
        let id = unsafe { CStr::from_ptr(elem.device_id) }
            .to_str()
            .unwrap()
            .to_owned();
        Self {
            id,
            revoked: elem.is_revoked,
        }
    }
}

/// The [attach_provisional_identity](crate::Core::attach_provisional_identity) function returns this struct.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct AttachResult {
    /// The provisional identity's status. Either `Ready` or `IdentityVerificationNeeded`.
    pub status: Status,
    /// A [VerificationMethod](crate::VerificationMethod) containing the email matching the created provisional identity
    pub verification_method: Option<VerificationMethod>,
}
