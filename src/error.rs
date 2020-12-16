use futures::io::ErrorKind;
use num_enum::FromPrimitive;
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Eq, PartialEq, FromPrimitive)]
#[non_exhaustive]
#[repr(u32)]
pub enum ErrorCode {
    NoError = 0,
    /// Developer error, one of the function's argument is invalid
    InvalidArgument = 1,
    /// Tanker internal error, thrown as a last resort
    InternalError = 2,
    /// Network error, e.g. connection lost or the Tanker server is not reachable
    NetworkError = 3,
    /// Developer error, a function's precondition was violated
    PreconditionFailed = 4,
    /// An asynchronous operation was canceled when Tanker stopped
    OperationCanceled = 5,
    /// A decryption operation failed
    DecryptionFailed = 6,
    /// The group would exceed the maximum member limit (1000)
    GroupTooBig = 7,
    /// An invalid identity verification was provided
    InvalidVerification = 8,
    /// There were too many attempts for that action. Please retry later.
    TooManyAttempts = 9,
    /// The identity verification is expired
    ExpiredVerification = 10,
    /// There was an error on a stream (see `source()` for more detail)
    IoError = 11,
    /// The current device is revoked and cannot be used anymore
    DeviceRevoked = 12,
    /// There was a conflict with a concurrent operation from another device/user. Please try again
    Conflict = 13,
    /// A new version of the SDK is required to perform the requested action
    UpgradeRequired = 14,

    #[num_enum(default)]
    UnknownError = u32::max_value(),
}

/// Every Tanker function that may fail returns a Result<_, Error> type.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Error {
    code: ErrorCode,
    msg: String,
    source: Option<Arc<dyn std::error::Error + Send + Sync>>,
}

impl Error {
    pub fn new(code: ErrorCode, msg: String) -> Self {
        Self {
            code,
            msg,
            source: None,
        }
    }

    pub fn new_with_source(
        code: ErrorCode,
        msg: String,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            code,
            msg,
            source: Some(Arc::new(source)),
        }
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.msg
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.msg)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|e| e.as_ref() as &_)
    }
}

impl From<Error> for futures::io::Error {
    fn from(e: Error) -> Self {
        let kind = match e.code() {
            ErrorCode::OperationCanceled => ErrorKind::Interrupted,
            ErrorCode::GroupTooBig | ErrorCode::InvalidArgument => ErrorKind::InvalidInput,
            ErrorCode::ExpiredVerification
            | ErrorCode::DeviceRevoked
            | ErrorCode::InvalidVerification => ErrorKind::PermissionDenied,
            ErrorCode::NetworkError => ErrorKind::ConnectionReset,
            ErrorCode::IoError => ErrorKind::BrokenPipe,
            ErrorCode::NoError
            | ErrorCode::InternalError
            | ErrorCode::PreconditionFailed
            | ErrorCode::DecryptionFailed
            | ErrorCode::TooManyAttempts
            | ErrorCode::Conflict
            | ErrorCode::UpgradeRequired
            | ErrorCode::UnknownError => ErrorKind::Other,
        };
        futures::io::Error::new(kind, e)
    }
}

impl From<futures::io::Error> for Error {
    fn from(e: futures::io::Error) -> Self {
        Error::new_with_source(
            ErrorCode::IoError,
            "IO error in Tanker stream".to_owned(),
            e,
        )
    }
}

#[cfg(test)]
mod tests {
    // This test checks that our Error type can be sent across threads
    // It contain no assertions because we just need it to compile
    #[test]
    fn errors_are_send_sync() {
        fn assert_traits<T: Send + Sync>() {}
        assert_traits::<super::Error>();
    }
}
