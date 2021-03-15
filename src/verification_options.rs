/// The `with_token` method requests to create a Session Token on verification
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct VerificationOptions {
    pub(crate) with_token: bool,
}

impl VerificationOptions {
    pub fn new() -> Self {
        Default::default()
    }

    /// Sets whether the encrypted data should be decryptable by the author
    pub fn with_token(mut self) -> Self {
        self.with_token = true;
        self
    }
}

impl Default for VerificationOptions {
    fn default() -> Self {
        Self { with_token: false }
    }
}
