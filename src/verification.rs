use crate::ctanker::{
    CEmailVerification, COIDCAuthorizationCodeVerification, CPhoneNumberVerification,
    CPreverifiedOIDCVerification, CVerification, CVerificationPtr,
};
use std::ffi::CString;

const CVERIFICATION_VERSION: u8 = 9;
const CEMAIL_VERIFICATION_VERSION: u8 = 1;
const CPHONE_NUMBER_VERIFICATION_VERSION: u8 = 1;
const CPREVERIFIED_OIDC_VERIFICATION_VERSION: u8 = 1;
const COIDC_AUTORIZATION_CODE_VERIFICATION_VERSION: u8 = 1;

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
    E2ePassphrase = 8,
    PreverifiedOIDC = 9,
    OIDCAuthorizationCode = 10,
    PrehashedAndEncryptedPassphrase = 11,
}

pub(crate) struct CVerificationWrapper {
    cstrings: Vec<CString>,
    cverif: CVerification,
}

// SAFETY: CVerificationWrapper is thread-safe (read-only after construction)
unsafe impl Send for CVerificationWrapper {}
unsafe impl Sync for CVerificationWrapper {}

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
                e2e_passphrase: std::ptr::null(),
                preverified_oidc_verification: CPreverifiedOIDCVerification {
                    version: CPREVERIFIED_OIDC_VERIFICATION_VERSION,
                    subject: std::ptr::null(),
                    provider_id: std::ptr::null(),
                },
                oidc_authorization_code_verification: COIDCAuthorizationCodeVerification {
                    version: COIDC_AUTORIZATION_CODE_VERIFICATION_VERSION,
                    provider_id: std::ptr::null(),
                    authorization_code: std::ptr::null(),
                    state: std::ptr::null(),
                },
                prehashed_and_encrypted_passphrase: std::ptr::null(),
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

    pub(self) fn with_e2e_passphrase(e2e_passphrase: &str) -> Self {
        let mut wrapper = Self::new();
        let cpass = CString::new(e2e_passphrase).unwrap();

        wrapper.cverif.verification_method_type = Type::E2ePassphrase as u8;
        wrapper.cverif.e2e_passphrase = cpass.as_ptr();

        wrapper.cstrings.push(cpass);
        wrapper
    }

    pub(self) fn with_prehashed_and_encrypted_passphrase(
        prehashed_and_encrypted_passphrase: &str,
    ) -> Self {
        let mut wrapper = Self::new();
        let cpaep = CString::new(prehashed_and_encrypted_passphrase).unwrap();

        wrapper.cverif.verification_method_type = Type::PrehashedAndEncryptedPassphrase as u8;
        wrapper.cverif.prehashed_and_encrypted_passphrase = cpaep.as_ptr();

        wrapper.cstrings.push(cpaep);
        wrapper
    }

    pub(self) fn with_preverifed_oidc(subject: &str, provider_id: &str) -> Self {
        let mut wrapper = Self::new();
        let csubject = CString::new(subject).unwrap();
        let cprovider_id = CString::new(provider_id).unwrap();

        wrapper.cverif.verification_method_type = Type::PreverifiedOIDC as u8;
        wrapper.cverif.preverified_oidc_verification.subject = csubject.as_ptr();
        wrapper.cverif.preverified_oidc_verification.provider_id = cprovider_id.as_ptr();

        wrapper.cstrings = vec![csubject, cprovider_id];
        wrapper
    }

    pub(self) fn with_oidc_authorization_code(
        provider_id: &str,
        authorization_code: &str,
        state: &str,
    ) -> Self {
        let mut wrapper = Self::new();
        let cprovider_id = CString::new(provider_id).unwrap();
        let cauthorization_code = CString::new(authorization_code).unwrap();
        let cstate = CString::new(state).unwrap();

        wrapper.cverif.verification_method_type = Type::OIDCAuthorizationCode as u8;
        wrapper
            .cverif
            .oidc_authorization_code_verification
            .provider_id = cprovider_id.as_ptr();
        wrapper
            .cverif
            .oidc_authorization_code_verification
            .authorization_code = cauthorization_code.as_ptr();
        wrapper.cverif.oidc_authorization_code_verification.state = cstate.as_ptr();

        wrapper.cstrings = vec![cprovider_id, cauthorization_code, cstate];
        wrapper
    }

    pub fn as_cverification_ptr(&self) -> CVerificationPtr {
        CVerificationPtr(&self.cverif)
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
    #[deprecated(
        since = "4.2.0",
        note = "The entire OIDC flow has been reworked, this verification method has been deprecated as a result, use Verification::OIDCAuthorizationCode instead"
    )]
    OIDCIDToken(String),
    PhoneNumber {
        phone_number: String,
        verification_code: String,
    },
    PreverifiedEmail(String),
    PreverifiedPhoneNumber(String),
    E2ePassphrase(String),
    PreverifiedOIDC {
        subject: String,
        provider_id: String,
    },
    OIDCAuthorizationCode {
        provider_id: String,
        authorization_code: String,
        state: String,
    },
    PrehashedAndEncryptedPassphrase(String),
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
            #[allow(deprecated)]
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
            Verification::E2ePassphrase(e2e_passphrase) => {
                CVerificationWrapper::with_e2e_passphrase(e2e_passphrase)
            }
            Verification::PreverifiedOIDC {
                subject,
                provider_id,
            } => CVerificationWrapper::with_preverifed_oidc(subject, provider_id),
            Verification::OIDCAuthorizationCode {
                provider_id,
                authorization_code,
                state,
            } => CVerificationWrapper::with_oidc_authorization_code(
                provider_id,
                authorization_code,
                state,
            ),
            Verification::PrehashedAndEncryptedPassphrase(prehashed_and_encrypted_passphrase) => {
                CVerificationWrapper::with_prehashed_and_encrypted_passphrase(
                    prehashed_and_encrypted_passphrase,
                )
            }
        }
    }
}
