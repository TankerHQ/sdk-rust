/// Extra options used during identity verification
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct VerificationOptions {
    pub(crate) with_session_token: bool,
}

impl VerificationOptions {
    pub fn new() -> Self {
        Default::default()
    }

    /// Requests to create a Session Token on verification
    pub fn with_session_token(mut self) -> Self {
        self.with_session_token = true;
        self
    }
}

impl Default for VerificationOptions {
    fn default() -> Self {
        Self {
            with_session_token: false,
        }
    }
}
