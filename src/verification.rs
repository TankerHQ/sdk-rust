use crate::ctanker::{CEmailVerification, CVerification};
use std::ffi::CString;

const CVERIFICATION_VERSION: u8 = 3;
const CEMAIL_VERIFICATION_VERSION: u8 = 1;

#[repr(u8)]
enum Type {
    Email = 1,
    Passphrase = 2,
    VerificationKey = 3,
    #[allow(clippy::upper_case_acronyms)]
    OIDCIDToken = 4,
}

pub(crate) struct CVerificationWrapper {
    cstrings: Vec<CString>,
    cverif: CVerification,
}

impl CVerificationWrapper {
    fn new() -> Self {
        Self {
            cstrings: vec![],
            cverif: CVerification {
                version: CVERIFICATION_VERSION,
                verification_method_type: 0,
                verification_key: std::ptr::null(),
                email_verification: CEmailVerification {
                    version: CEMAIL_VERIFICATION_VERSION,
                    email: std::ptr::null(),
                    verification_code: std::ptr::null(),
                },
                passphrase: std::ptr::null(),
                oidc_id_token: std::ptr::null(),
            },
        }
    }

    pub(self) fn with_email(email: &str, verif_code: &str) -> Self {
        let mut wrapper = Self::new();
        let cemail = CString::new(email).unwrap();
        let cverif_code = CString::new(verif_code).unwrap();

        wrapper.cverif.verification_method_type = Type::Email as u8;
        wrapper.cverif.email_verification.email = cemail.as_ptr();
        wrapper.cverif.email_verification.verification_code = cverif_code.as_ptr();

        wrapper.cstrings.push(cemail);
        wrapper.cstrings.push(cverif_code);
        wrapper
    }

    pub(self) fn with_passphrase(passphrase: &str) -> Self {
        let mut wrapper = Self::new();
        let cpass = CString::new(passphrase).unwrap();

        wrapper.cverif.verification_method_type = Type::Passphrase as u8;
        wrapper.cverif.passphrase = cpass.as_ptr();

        wrapper.cstrings.push(cpass);
        wrapper
    }

    pub(self) fn with_verification_key(key: &str) -> Self {
        let mut wrapper = Self::new();
        let ckey = CString::new(key).unwrap();

        wrapper.cverif.verification_method_type = Type::VerificationKey as u8;
        wrapper.cverif.verification_key = ckey.as_ptr();

        wrapper.cstrings.push(ckey);
        wrapper
    }

    pub(self) fn with_oidc_id_token(token: &str) -> Self {
        let mut wrapper = Self::new();
        let ctoken = CString::new(token).unwrap();

        wrapper.cverif.verification_method_type = Type::OIDCIDToken as u8;
        wrapper.cverif.oidc_id_token = ctoken.as_ptr();

        wrapper.cstrings.push(ctoken);
        wrapper
    }

    pub fn as_cverification(&self) -> &CVerification {
        &self.cverif
    }
}

/// A `Verification` object is typically used to perform an identity verification, or to register a new identity verification method.
#[non_exhaustive]
pub enum Verification {
    Email {
        email: String,
        verification_code: String,
    },
    Passphrase(String),
    VerificationKey(String),
    #[allow(clippy::upper_case_acronyms)]
    OIDCIDToken(String),
}

impl Verification {
    pub(crate) fn to_cverification_wrapper(&self) -> CVerificationWrapper {
        match &self {
            Verification::Email {
                email,
                verification_code,
            } => CVerificationWrapper::with_email(&email, &verification_code),
            Verification::Passphrase(passphrase) => {
                CVerificationWrapper::with_passphrase(&passphrase)
            }
            Verification::VerificationKey(key) => CVerificationWrapper::with_verification_key(&key),
            Verification::OIDCIDToken(token) => CVerificationWrapper::with_oidc_id_token(&token),
        }
    }
}
