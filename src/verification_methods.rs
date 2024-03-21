use crate::ctanker::CVerificationMethod;
use crate::{Error, ErrorCode};
use num_enum::FromPrimitive;
use std::ffi::CStr;

#[cfg(test)]
use variant_count::VariantCount;

/// `VerificationMethod` instances are returned by functions that list verification methods available for an upcoming identity verification.
#[non_exhaustive]
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(test, derive(VariantCount))]
pub enum VerificationMethod {
    Email(String),
    Passphrase,
    VerificationKey,
    #[allow(clippy::upper_case_acronyms)]
    OIDCIDToken {
        provider_id: String,
        provider_display_name: String,
    },
    PhoneNumber(String),
    PreverifiedEmail(String),
    PreverifiedPhoneNumber(String),
    E2ePassphrase,
    // PreverifiedOIDC is not exposed as a VerificationMethod
    // OIDCAuthorizationCode is not exposed as a VerificationMethod
}

#[derive(FromPrimitive)]
#[repr(u8)]
#[cfg_attr(test, derive(VariantCount))]
enum CMethodType {
    Email = 1,
    Passphrase = 2,
    VerificationKey = 3,
    #[allow(clippy::upper_case_acronyms)]
    OIDCIDToken = 4,
    PhoneNumber = 5,
    PreverifiedEmail = 6,
    PreverifiedPhoneNumber = 7,
    E2ePassphrase = 8,
    // PreverifiedOIDC = 9, PreverifiedOIDC is not exposed as a VerificationMethod
    // OIDCAuthorizationCode = 10, OIDCAuthorizationCode is not exposed as a VerificationMethod
    #[num_enum(default)]
    Invalid,
}

impl VerificationMethod {
    pub(crate) fn try_from(method: &CVerificationMethod) -> Result<Self, Error> {
        let ctype = method.verification_method_type;
        match ctype.into() {
            CMethodType::Email => {
                // SAFETY: If we get a valid Email verification method, the email is a valid string
                let c_email = unsafe { CStr::from_ptr(method.value1) };
                let email = c_email.to_str().unwrap().into();
                Ok(VerificationMethod::Email(email))
            }
            CMethodType::Passphrase => Ok(VerificationMethod::Passphrase),
            CMethodType::VerificationKey => Ok(VerificationMethod::VerificationKey),
            CMethodType::OIDCIDToken => {
                // SAFETY: If we get a valid OIDC verification method, the values are valid strings
                let c_prov_id = unsafe { CStr::from_ptr(method.value1) };
                let provider_id = c_prov_id.to_str().unwrap().into();
                let c_prov_name = unsafe { CStr::from_ptr(method.value2) };
                let provider_display_name = c_prov_name.to_str().unwrap().into();
                Ok(VerificationMethod::OIDCIDToken {
                    provider_id,
                    provider_display_name,
                })
            }
            CMethodType::PhoneNumber => {
                // SAFETY: If we get a valid PhoneNumber verification method, the number is a valid string
                let c_phone_number = unsafe { CStr::from_ptr(method.value1) };
                let phone_number = c_phone_number.to_str().unwrap().into();
                Ok(VerificationMethod::PhoneNumber(phone_number))
            }
            CMethodType::PreverifiedEmail => {
                // SAFETY: If we get a valid Email verification method, the email is a valid string
                let c_preverified_email = unsafe { CStr::from_ptr(method.value1) };
                let preverified_email = c_preverified_email.to_str().unwrap().into();
                Ok(VerificationMethod::PreverifiedEmail(preverified_email))
            }
            CMethodType::PreverifiedPhoneNumber => {
                // SAFETY: If we get a valid PhoneNumber verification method, the number is a valid string
                let c_preverified_phone_number = unsafe { CStr::from_ptr(method.value1) };
                let preverified_phone_number = c_preverified_phone_number.to_str().unwrap().into();
                Ok(VerificationMethod::PreverifiedPhoneNumber(
                    preverified_phone_number,
                ))
            }
            CMethodType::E2ePassphrase => Ok(VerificationMethod::E2ePassphrase),
            CMethodType::Invalid => Err(Error::new(
                ErrorCode::InternalError,
                format!("Invalid verification method type {ctype}"),
            )),
        }
    }
}

#[cfg(test)]
mod test {
    use super::{CMethodType, VerificationMethod};

    #[test]
    fn verification_method_variants_up_to_date() {
        // Makes sure both enums are updated and kept in sync with each other
        // (which also indirectly checks that `try_from` is updated!)
        assert_eq!(
            VerificationMethod::VARIANT_COUNT,
            CMethodType::VARIANT_COUNT - 1
        );
    }
}
