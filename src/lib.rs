// Our unused_unsafe are not really unused, we follow the RFC 2585 unsafe-in-unsafe-fn changes,
// however unsafe_op_in_unsafe_fn is still an unstable feature so silence this warning for now.
// Tracking issue for stabilization: https://github.com/rust-lang/rust/issues/71668
#![allow(unused_unsafe)]

mod core;
pub use self::core::Core;

mod encryption_session;
pub use encryption_session::EncryptionSession;

mod error;
pub use error::{Error, ErrorCode};

mod types;
pub use types::*;

mod sharing_options;
pub use sharing_options::{EncryptionOptions, Padding, SharingOptions};

mod verification_options;
pub use verification_options::VerificationOptions;

mod verification_methods;
pub use verification_methods::*;

mod verification;
pub use verification::*;

mod ctanker;
