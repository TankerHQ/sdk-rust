/// Extra options used during identity verification
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct VerificationOptions {
    pub(crate) with_session_token: bool,
    pub(crate) allow_e2e_method_switch: bool,
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

    /// Allow switching to and from E2E verification methods
    pub fn allow_e2e_method_switch(mut self) -> Self {
        self.allow_e2e_method_switch = true;
        self
    }
}

#[allow(clippy::derivable_impls)] // The Defaults for these options are not obvious
impl Default for VerificationOptions {
    fn default() -> Self {
        Self {
            with_session_token: false,
            allow_e2e_method_switch: false,
        }
    }
}
