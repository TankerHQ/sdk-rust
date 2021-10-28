use crate::ctanker::{CEmailVerification, CPhoneNumberVerification, CVerification};
use std::ffi::CString;

const CVERIFICATION_VERSION: u8 = 5;
const CEMAIL_VERIFICATION_VERSION: u8 = 1;
const CPHONE_NUMBER_VERIFICATION_VERSION: u8 = 1;

#[repr(u8)]
enum Type {
    Email = 1,
    Passphrase = 2,
    VerificationKey = 3,
    #[allow(clippy::upper_case_acronyms)]
    OIDCIDToken = 4,
    PhoneNumber = 5,
    PreverifiedEmail = 6,
    PreverifiedPhoneNumber = 7,
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
                phone_number_verification: CPhoneNumberVerification {
                    version: CPHONE_NUMBER_VERIFICATION_VERSION,
                    phone_number: std::ptr::null(),
                    verification_code: std::ptr::null(),
                },
                preverified_email: std::ptr::null(),
                preverified_phone_number: std::ptr::null(),
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

    pub(self) fn with_phone_number(phone_number: &str, verif_code: &str) -> Self {
        let mut wrapper = Self::new();
        let cphone_number = CString::new(phone_number).unwrap();
        let cverif_code = CString::new(verif_code).unwrap();

        wrapper.cverif.verification_method_type = Type::PhoneNumber as u8;
        wrapper.cverif.phone_number_verification.phone_number = cphone_number.as_ptr();
        wrapper.cverif.phone_number_verification.verification_code = cverif_code.as_ptr();

        wrapper.cstrings.push(cphone_number);
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

    pub(self) fn with_preverified_email(preverified_email: &str) -> Self {
        let mut wrapper = Self::new();
        let cpreverified_email = CString::new(preverified_email).unwrap();

        wrapper.cverif.verification_method_type = Type::PreverifiedEmail as u8;
        wrapper.cverif.preverified_email = cpreverified_email.as_ptr();

        wrapper.cstrings.push(cpreverified_email);
        wrapper
    }

    pub(self) fn with_preverified_phone_number(preverified_phone_number: &str) -> Self {
        let mut wrapper = Self::new();
        let cpreverified_phone_number = CString::new(preverified_phone_number).unwrap();

        wrapper.cverif.verification_method_type = Type::PreverifiedPhoneNumber as u8;
        wrapper.cverif.preverified_phone_number = cpreverified_phone_number.as_ptr();

        wrapper.cstrings.push(cpreverified_phone_number);
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
    PhoneNumber {
        phone_number: String,
        verification_code: String,
    },
    PreverifiedEmail(String),
    PreverifiedPhoneNumber(String),
}

impl Verification {
    pub(crate) fn to_cverification_wrapper(&self) -> CVerificationWrapper {
        match &self {
            Verification::Email {
                email,
                verification_code,
            } => CVerificationWrapper::with_email(email, verification_code),
            Verification::Passphrase(passphrase) => {
                CVerificationWrapper::with_passphrase(passphrase)
            }
            Verification::VerificationKey(key) => CVerificationWrapper::with_verification_key(key),
            Verification::OIDCIDToken(token) => CVerificationWrapper::with_oidc_id_token(token),
            Verification::PhoneNumber {
                phone_number,
                verification_code,
            } => CVerificationWrapper::with_phone_number(phone_number, verification_code),
            Verification::PreverifiedEmail(preverified_email) => {
                CVerificationWrapper::with_preverified_email(preverified_email)
            }
            Verification::PreverifiedPhoneNumber(preverified_phone_number) => {
                CVerificationWrapper::with_preverified_phone_number(preverified_phone_number)
            }
        }
    }
}
