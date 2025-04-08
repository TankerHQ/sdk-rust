use crate::VerificationMethod;
use num_enum::{TryFromPrimitive, UnsafeFromPrimitive};
use std::ffi::CString;
use std::fmt::{Display, Formatter};

/// Options used by the [Core](struct.Core.html) struct
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Options {
    pub(crate) url: Option<CString>,
    pub(crate) app_id: CString,
    pub(crate) persistent_path: CString,
    pub(crate) cache_path: CString,
    pub(crate) sdk_type: Option<CString>,
}

impl Options {
    #[allow(clippy::doc_overindented_list_items)]
    /// # Arguments
    /// * `app_id` - Your Tanker App ID
    /// * `persistent_path` - A writable folder. Tanker will use this folder to
    ///    store persistent data about user sessions on the current device.
    /// * `cache_path` - A writable folder. Tanker will use this folder to
    ///    store encrypted cached keys. May be the same as `persistent_path`.
    pub fn new(app_id: String, persistent_path: String, cache_path: String) -> Self {
        Self {
            url: None,
            app_id: CString::new(app_id).unwrap(),
            persistent_path: CString::new(persistent_path).unwrap(),
            cache_path: CString::new(cache_path).unwrap(),
            sdk_type: None,
        }
    }

    pub fn with_url(mut self, url: String) -> Self {
        self.url = Some(CString::new(url).unwrap());
        self
    }

    pub fn with_sdk_type(mut self, sdk_type: String) -> Self {
        self.sdk_type = Some(CString::new(sdk_type).unwrap());
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

/// The [attach_provisional_identity](crate::Core::attach_provisional_identity) function returns this struct.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct AttachResult {
    /// The provisional identity's status. Either `Ready` or `IdentityVerificationNeeded`.
    pub status: Status,
    /// A [VerificationMethod](crate::VerificationMethod) containing the email matching the created provisional identity
    pub verification_method: Option<VerificationMethod>,
}
