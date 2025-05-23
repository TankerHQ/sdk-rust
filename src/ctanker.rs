// C types can vary by target arch, so casts that look unnecessary on x86 are actually needed
#![allow(clippy::unnecessary_cast)]

#[cfg(target_family = "windows")]
macro_rules! tanker_call {
    ($self:ident, $func_name:ident($($args:tt)*)) => { $self.ctanker_api.$func_name($($args)*) };
}
#[cfg(target_family = "windows")]
macro_rules! tanker_call_ext {
    ($func_name:ident($($args:tt)*)) => { CTankerLib::get().ctanker_api.$func_name($($args)*) };
}

#[cfg(not(target_family = "windows"))]
macro_rules! tanker_call {
    ($self:ident, $func_name:ident($($args:tt)*)) => { $func_name($($args)*) };
}
#[cfg(not(target_family = "windows"))]
macro_rules! tanker_call_ext {
    ($func_name:ident($($args:tt)*)) => { $func_name($($args)*) };
}

mod cfuture;
pub(crate) use cfuture::*;

#[cfg(feature = "http")]
pub mod chttp;
#[cfg(feature = "http")]
pub(crate) use chttp::*;

mod cstream;
pub use cstream::*;

use crate::http::HttpClient;
use crate::{
    AttachResult, EncryptionOptions, Error, ErrorCode, LogRecord, LogRecordLevel, Options, Padding,
    SharingOptions, Status, VerificationMethod, VerificationOptions,
};
use lazy_static::lazy_static;
use num_enum::UnsafeFromPrimitive;
use std::convert::TryFrom;
use std::ffi::{c_void, CStr, CString};
use std::marker::PhantomData;
use std::os::raw::c_char;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex, Once};

use self::bindings::*;

pub type CVerification = tanker_verification;
pub type CEmailVerification = tanker_email_verification;
pub type CPhoneNumberVerification = tanker_phone_number_verification;
pub type COIDCAuthorizationCodeVerification = tanker_oidc_authorization_code_verification;
pub type CVerificationMethod = tanker_verification_method;
pub type CPreverifiedOIDCVerification = tanker_preverified_oidc_verification;
pub type CEncSessPtr = *mut tanker_encryption_session_t;
pub type LogHandlerCallback = Box<dyn Fn(LogRecord) + Send>;

#[derive(Copy, Clone, Debug)]
pub struct CTankerPtr(pub(crate) *mut tanker_t);

// SAFETY: ctanker is thread-safe
unsafe impl Send for CTankerPtr {}

#[derive(Copy, Clone, Debug)]
pub struct CVerificationPtr(pub(crate) *const tanker_verification);

// SAFETY: ctanker is thread-safe
unsafe impl Send for CVerificationPtr {}

// SAFETY: ctanker is thread-safe
unsafe impl Send for tanker_http_options {}

// SAFETY: ctanker is thread-safe
unsafe impl Sync for CTankerLib {}

pub(crate) static RUST_SDK_VERSION: &str = env!("CARGO_PKG_VERSION");
pub(crate) static RUST_SDK_TYPE: &str = "client-rust";

static TANKER_INITIALIZED: Once = Once::new();
lazy_static! {
    static ref LOG_HANDLER_CALLBACK: Mutex<Option<LogHandlerCallback>> = Mutex::new(None);
}
#[cfg(target_family = "windows")]
lazy_static! {
    static ref CTANKER_API: CTankerLib = CTankerLib {
        ctanker_api: unsafe { ctanker_api::new("ctanker.dll").unwrap() }
    };
}
#[cfg(not(target_family = "windows"))]
lazy_static! {
    static ref CTANKER_API: CTankerLib = CTankerLib {};
}

mod bindings {
    #![allow(dead_code)] // Autogenerated code in ctanker.rs may be unused
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(clippy::redundant_static_lifetimes)]
    #![allow(clippy::unused_unit)]
    include!(concat!(env!("NATIVE_BINDINGS_FOLDER"), "/ctanker.rs"));
}

unsafe extern "C" fn log_handler_thunk(clog: *const tanker_log_record) {
    let global_callback_guard = LOG_HANDLER_CALLBACK.lock().unwrap();
    let global_callback = global_callback_guard.as_ref();
    let callback = match global_callback {
        None => return,
        Some(cb) => cb,
    };

    // SAFETY: The native SDK always sends valid log records
    let record = unsafe {
        let category = CStr::from_ptr((*clog).category);
        let file = CStr::from_ptr((*clog).file);
        let message = CStr::from_ptr((*clog).message);
        let level = LogRecordLevel::try_from((*clog).level).unwrap();
        LogRecord {
            category: category.to_str().unwrap().to_string(),
            level,
            file: file.to_str().unwrap().to_string(),
            line: (*clog).line,
            message: message.to_str().unwrap().to_string(),
        }
    };
    callback(record);
}

pub struct CTankerLib {
    #[cfg(target_family = "windows")]
    ctanker_api: ctanker_api,
}

impl CTankerLib {
    pub fn version_string(&self) -> &'static str {
        let c_version = unsafe { CStr::from_ptr(tanker_call!(self, tanker_version_string())) };
        c_version
            .to_str()
            .expect("tanker native version expected to be valid UTF-8")
    }

    pub fn init() {
        unsafe {
            // SAFETY: tanker_set_log_handler must be called once, and before any Tanker logs
            TANKER_INITIALIZED.call_once(|| {
                tanker_call_ext!(tanker_set_log_handler(Some(log_handler_thunk)));
                tanker_call_ext!(tanker_init());
            });
        }
    }

    pub fn get() -> &'static CTankerLib {
        &CTANKER_API
    }

    pub unsafe fn set_log_handler(callback: LogHandlerCallback) {
        let mut global_callback = LOG_HANDLER_CALLBACK.lock().unwrap();
        *global_callback = Some(callback);
    }

    pub async fn create(
        &self,
        options: Options,
        http_client: Option<Arc<HttpClient>>,
    ) -> Result<CTankerPtr, Error> {
        let sdk_type = options
            .sdk_type
            .unwrap_or_else(|| CString::new(RUST_SDK_TYPE).unwrap());
        let sdk_version = CString::new(RUST_SDK_VERSION).unwrap();

        let http_options = match http_client {
            #[cfg(feature = "http")]
            Some(client) => tanker_http_options {
                send_request: Some(chttp::send_http_request),
                cancel_request: Some(chttp::cancel_http_request),
                data: Arc::as_ptr(&client) as *mut c_void,
            },
            _ => tanker_http_options {
                send_request: None,
                cancel_request: None,
                data: std::ptr::null_mut(),
            },
        };

        let fut = {
            let coptions = tanker_options {
                version: 4,
                app_id: options.app_id.as_ptr(),
                url: options
                    .url
                    .as_ref()
                    .map(|s| s.as_ptr())
                    .unwrap_or(std::ptr::null()),
                persistent_path: options.persistent_path.as_ptr(),
                cache_path: options.cache_path.as_ptr(),
                sdk_type: sdk_type.as_ptr(),
                sdk_version: sdk_version.as_ptr(),
                http_options,
                datastore_options: tanker_datastore_options {
                    open: None,
                    close: None,
                    nuke: None,
                    put_serialized_device: None,
                    find_serialized_device: None,
                    put_cache_values: None,
                    find_cache_values: None,
                },
            };
            unsafe { CFuture::new(tanker_call!(self, tanker_create(&coptions))) }
        };
        fut.await
    }

    pub async unsafe fn destroy(&self, ctanker: CTankerPtr) {
        let fut = unsafe { CFuture::new(tanker_call!(self, tanker_destroy(ctanker.0))) };
        let _: Result<(), _> = fut.await; // Ignore errors, nothing useful we can do if destroy() fails
    }

    pub unsafe fn status(&self, ctanker: CTankerPtr) -> Status {
        let status = unsafe { tanker_call!(self, tanker_status(ctanker.0)) };
        // SAFETY: The native lib never returns invalid status codes
        unsafe { Status::unchecked_transmute_from(status as u32) }
    }

    pub async unsafe fn start(&self, ctanker: CTankerPtr, identity: &str) -> Result<Status, Error> {
        let cidentity = CString::new(identity).map_err(|_| {
            Error::new(
                ErrorCode::InvalidArgument,
                "identity is not a valid CString".into(),
            )
        })?;
        let fut = unsafe {
            CFuture::<u32>::new(tanker_call!(
                self,
                tanker_start(ctanker.0, cidentity.as_ptr())
            ))
        };
        fut.await.map(|status_voidptr| {
            // SAFETY: The native lib never returns invalid status codes
            unsafe { Status::unchecked_transmute_from(status_voidptr) }
        })
    }

    pub async unsafe fn stop(&self, ctanker: CTankerPtr) -> Result<(), Error> {
        let fut = unsafe { CFuture::new(tanker_call!(self, tanker_stop(ctanker.0))) };
        fut.await
    }

    pub async unsafe fn create_oidc_nonce(&self, ctanker: CTankerPtr) -> Result<String, Error> {
        let fut = unsafe { CFuture::new(tanker_call!(self, tanker_create_oidc_nonce(ctanker.0))) };
        fut.await
    }

    pub async unsafe fn set_oidc_test_nonce(
        &self,
        ctanker: CTankerPtr,
        nonce: &str,
    ) -> Result<(), Error> {
        let cnonce = CString::new(nonce).map_err(|_| {
            Error::new(
                ErrorCode::InvalidArgument,
                "nonce is not a valid CString".into(),
            )
        })?;

        let fut = unsafe {
            CFuture::new(tanker_call!(
                self,
                tanker_set_oidc_test_nonce(ctanker.0, cnonce.as_ptr())
            ))
        };
        fut.await
    }

    pub async unsafe fn generate_verification_key(
        &self,
        ctanker: CTankerPtr,
    ) -> Result<String, Error> {
        let fut = unsafe {
            CFuture::new(tanker_call!(
                self,
                tanker_generate_verification_key(ctanker.0)
            ))
        };
        fut.await
    }

    pub async unsafe fn register_identity(
        &self,
        ctanker: CTankerPtr,
        verification: CVerificationPtr,
        options: &VerificationOptions,
    ) -> Result<Option<String>, Error> {
        let c_options = tanker_verification_options {
            version: 2,
            with_session_token: options.with_session_token,
            allow_e2e_method_switch: options.allow_e2e_method_switch,
        };
        let fut = unsafe {
            CFuture::new(tanker_call!(
                self,
                tanker_register_identity(ctanker.0, verification.0, &c_options,)
            ))
        };
        fut.await
    }

    pub async unsafe fn verify_identity(
        &self,
        ctanker: CTankerPtr,
        verification: CVerificationPtr,
        options: &VerificationOptions,
    ) -> Result<Option<String>, Error> {
        let c_options = tanker_verification_options {
            version: 2,
            with_session_token: options.with_session_token,
            allow_e2e_method_switch: options.allow_e2e_method_switch,
        };
        let fut = unsafe {
            CFuture::new(tanker_call!(
                self,
                tanker_verify_identity(ctanker.0, verification.0, &c_options,)
            ))
        };
        fut.await
    }

    pub async unsafe fn verify_provisional_identity(
        &self,
        ctanker: CTankerPtr,
        verif: CVerificationPtr,
    ) -> Result<(), Error> {
        let fut = unsafe {
            CFuture::new(tanker_call!(
                self,
                tanker_verify_provisional_identity(ctanker.0, verif.0)
            ))
        };
        fut.await
    }

    pub async unsafe fn set_verification_method(
        &self,
        ctanker: CTankerPtr,
        verification: CVerificationPtr,
        options: &VerificationOptions,
    ) -> Result<Option<String>, Error> {
        let c_options = tanker_verification_options {
            version: 2,
            with_session_token: options.with_session_token,
            allow_e2e_method_switch: options.allow_e2e_method_switch,
        };
        let fut = unsafe {
            CFuture::new(tanker_call!(
                self,
                tanker_set_verification_method(ctanker.0, verification.0, &c_options,)
            ))
        };
        fut.await
    }

    pub async unsafe fn get_verification_methods(
        &self,
        ctanker: CTankerPtr,
    ) -> Result<Vec<VerificationMethod>, Error> {
        let fut = unsafe {
            CFuture::<*mut tanker_verification_method_list>::new(tanker_call!(
                self,
                tanker_get_verification_methods(ctanker.0)
            ))
        };
        let list: &mut tanker_verification_method_list = unsafe { &mut *fut.await? };
        let methods = std::slice::from_raw_parts(list.methods, list.count as usize)
            .iter()
            .map(VerificationMethod::try_from)
            .collect();

        unsafe { self.free_verification_method_list(list) };
        methods
    }

    pub async unsafe fn encrypt(
        &self,
        ctanker: CTankerPtr,
        data: &[u8],
        options: &EncryptionOptions,
    ) -> Result<Vec<u8>, Error> {
        let options_wrapper = options.to_c_encryption_options();

        let encrypted_size = tanker_call!(
            self,
            tanker_encrypted_size(data.len() as u64, options_wrapper.c_options.padding_step)
        ) as usize;

        let mut encrypted = Vec::with_capacity(encrypted_size);

        let fut = unsafe {
            CFuture::<()>::new(tanker_call!(
                self,
                tanker_encrypt(
                    ctanker.0,
                    encrypted.as_mut_ptr(),
                    data.as_ptr(),
                    data.len() as u64,
                    &options_wrapper.c_options,
                )
            ))
        };
        fut.await?;

        // SAFETY: If tanker_encrypt succeeds, it guarantees to have written encrypted_size bytes
        unsafe { encrypted.set_len(encrypted_size) };

        Ok(encrypted)
    }

    pub async unsafe fn decrypt(&self, ctanker: CTankerPtr, data: &[u8]) -> Result<Vec<u8>, Error> {
        let decrypted_size = unsafe {
            let fut = CFuture::new(tanker_call!(
                self,
                tanker_decrypted_size(data.as_ptr(), data.len() as u64,)
            ));
            fut.await?
        };
        let mut decrypted = Vec::with_capacity(decrypted_size);

        let clear_size = unsafe {
            CFuture::new(tanker_call!(
                self,
                tanker_decrypt(
                    ctanker.0,
                    decrypted.as_mut_ptr(),
                    data.as_ptr(),
                    data.len() as u64,
                )
            ))
        }
        .await?;

        // SAFETY: If tanker_decrypt succeeds, it guarantees to have written decrypted_size bytes
        unsafe { decrypted.set_len(clear_size) };

        Ok(decrypted)
    }

    pub async unsafe fn share(
        &self,
        ctanker: CTankerPtr,
        resource_ids: &[CString],
        options: &SharingOptions,
    ) -> Result<(), Error> {
        let fut = {
            let resource_ids = resource_ids.iter().map(|u| u.as_ptr()).collect::<Vec<_>>();
            let share_with_users = options
                .share_with_users
                .iter()
                .map(|u| u.as_ptr())
                .collect::<Vec<_>>();
            let share_with_groups = options
                .share_with_groups
                .iter()
                .map(|u| u.as_ptr())
                .collect::<Vec<_>>();

            let coptions = tanker_sharing_options {
                version: 1,
                share_with_users: share_with_users.as_ptr(),
                nb_users: share_with_users.len() as u32,
                share_with_groups: share_with_groups.as_ptr(),
                nb_groups: share_with_groups.len() as u32,
            };

            unsafe {
                CFuture::new(tanker_call!(
                    self,
                    tanker_share(
                        ctanker.0,
                        resource_ids.as_ptr(),
                        resource_ids.len() as u64,
                        &coptions,
                    )
                ))
            }
        };
        fut.await
    }

    pub async unsafe fn attach_provisional_identity(
        &self,
        ctanker: CTankerPtr,
        identity: &CStr,
    ) -> Result<AttachResult, Error> {
        let fut = unsafe {
            CFuture::<*mut tanker_attach_result>::new(tanker_call!(
                self,
                tanker_attach_provisional_identity(ctanker.0, identity.as_ptr(),)
            ))
        };
        let cresult: &mut tanker_attach_result = unsafe { &mut *fut.await? };
        let verification_method = if cresult.method.is_null() {
            None
        } else {
            // SAFETY: If method is non-null, it is a valid CVerificationMethod pointer
            let cmethod = unsafe { &mut *cresult.method };
            Some(VerificationMethod::try_from(cmethod)?)
        };
        let result = AttachResult {
            status: unsafe { Status::unchecked_transmute_from(cresult.status as u32) },
            verification_method,
        };

        unsafe { tanker_call!(self, tanker_free_attach_result(cresult)) };
        Ok(result)
    }

    pub async fn get_resource_id(&self, data: &[u8]) -> Result<String, Error> {
        let fut = unsafe {
            CFuture::new(tanker_call!(
                self,
                tanker_get_resource_id(data.as_ptr(), data.len() as u64,)
            ))
        };
        fut.await
    }

    pub async unsafe fn create_group(
        &self,
        ctanker: CTankerPtr,
        members: &[CString],
    ) -> Result<String, Error> {
        let member_ptrs = members.iter().map(|u| u.as_ptr()).collect::<Vec<_>>();

        let fut = unsafe {
            CFuture::new(tanker_call!(
                self,
                tanker_create_group(ctanker.0, member_ptrs.as_ptr(), member_ptrs.len() as u64,)
            ))
        };
        fut.await
    }

    pub async unsafe fn update_group_members(
        &self,
        ctanker: CTankerPtr,
        group_id: &CStr,
        users_to_add: &[CString],
        users_to_remove: &[CString],
    ) -> Result<(), Error> {
        let users_to_add = users_to_add.iter().map(|u| u.as_ptr()).collect::<Vec<_>>();
        let users_to_remove = users_to_remove
            .iter()
            .map(|u| u.as_ptr())
            .collect::<Vec<_>>();

        let fut = unsafe {
            CFuture::new(tanker_call!(
                self,
                tanker_update_group_members(
                    ctanker.0,
                    group_id.as_ptr(),
                    users_to_add.as_ptr(),
                    users_to_add.len() as u64,
                    users_to_remove.as_ptr(),
                    users_to_remove.len() as u64,
                )
            ))
        };
        fut.await
    }

    #[cfg(feature = "experimental-oidc")]
    pub async unsafe fn authenticate_with_idp(
        &self,
        ctanker: CTankerPtr,
        provider_id: &CStr,
        cookie: &CStr,
    ) -> Result<crate::Verification, Error> {
        let fut = unsafe {
            CFuture::<*mut tanker_oidc_authorization_code_verification>::new(tanker_call!(
                self,
                tanker_authenticate_with_idp(ctanker.0, provider_id.as_ptr(), cookie.as_ptr(),)
            ))
        };
        let cresult: &mut tanker_oidc_authorization_code_verification = unsafe { &mut *fut.await? };

        // SAFETY: If we get a valid OIDCAuthorizationCode verification method, every field is a valid string
        let c_authorization_code = unsafe { CStr::from_ptr(cresult.authorization_code) };
        let authorization_code = c_authorization_code.to_str().unwrap().into();
        let c_state = unsafe { CStr::from_ptr(cresult.state) };
        let state = c_state.to_str().unwrap().into();

        unsafe { tanker_call!(self, tanker_free_authenticate_with_idp_result(cresult)) }

        Ok(crate::Verification::OIDCAuthorizationCode {
            provider_id: provider_id.to_str().unwrap().into(),
            authorization_code,
            state,
        })
    }

    pub async unsafe fn encryption_session_open(
        &self,
        ctanker: CTankerPtr,
        options: &EncryptionOptions,
    ) -> Result<CEncSessPtr, Error> {
        let options_wrapper = options.to_c_encryption_options();

        unsafe {
            CFuture::new(tanker_call!(
                self,
                tanker_encryption_session_open(ctanker.0, &options_wrapper.c_options,)
            ))
        }
        .await
    }

    pub async unsafe fn encryption_session_encrypt(
        &self,
        csess: CEncSessPtr,
        data: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let encrypted_size = tanker_call!(
            self,
            tanker_encryption_session_encrypted_size(csess, data.len() as u64)
        ) as usize;
        let mut encrypted = Vec::with_capacity(encrypted_size);

        unsafe {
            CFuture::<()>::new(tanker_call!(
                self,
                tanker_encryption_session_encrypt(
                    csess,
                    encrypted.as_mut_ptr(),
                    data.as_ptr(),
                    data.len() as u64,
                )
            ))
        }
        .await?;

        // SAFETY: If encrypt succeeds, it guarantees to have written encrypted_size bytes
        unsafe { encrypted.set_len(encrypted_size) };

        Ok(encrypted)
    }

    pub async fn encryption_session_get_resource_id(&self, csess: CEncSessPtr) -> String {
        let fut = unsafe {
            CFuture::new(tanker_call!(
                self,
                tanker_encryption_session_get_resource_id(csess)
            ))
        };
        fut.await.unwrap()
    }

    pub async unsafe fn encryption_session_close(&self, csess: CEncSessPtr) -> Result<(), Error> {
        let fut =
            unsafe { CFuture::new(tanker_call!(self, tanker_encryption_session_close(csess))) };
        fut.await
    }

    pub async fn prehash_password(&self, pass: &str) -> Result<String, Error> {
        let cpass = CString::new(pass).map_err(|_| {
            Error::new(
                ErrorCode::InvalidArgument,
                "password is not a valid CString".into(),
            )
        })?;
        let fut =
            unsafe { CFuture::new(tanker_call!(self, tanker_prehash_password(cpass.as_ptr()))) };
        fut.await
    }

    pub unsafe fn free_buffer(&self, buffer: *const c_void) {
        unsafe { tanker_call!(self, tanker_free_buffer(buffer)) }
    }

    unsafe fn free_verification_method_list(&self, list: &mut tanker_verification_method_list) {
        unsafe { tanker_call!(self, tanker_free_verification_method_list(list)) }
    }
}

struct EncryptionOptionsWrapper<'a> {
    _share_with_users: Vec<*const c_char>,
    _share_with_groups: Vec<*const c_char>,
    c_options: tanker_encrypt_options,
    phantom: PhantomData<&'a ()>,
}

// SAFETY: Encryption options are thread-safe (read-only after construction)
unsafe impl Send for EncryptionOptionsWrapper<'_> {}

impl EncryptionOptions {
    fn to_c_encryption_options<'a>(&'a self) -> EncryptionOptionsWrapper<'a> {
        let share_with_users = self
            .share_with_users
            .iter()
            .map(|u| u.as_ptr())
            .collect::<Vec<_>>();
        let share_with_groups = self
            .share_with_groups
            .iter()
            .map(|u| u.as_ptr())
            .collect::<Vec<_>>();

        let c_padding = match self.padding_step {
            Padding::Auto => 0,
            Padding::Off => 1,
            Padding::Step(padding_step) => padding_step.as_value(),
        };

        let c_options = tanker_encrypt_options {
            version: 4,
            share_with_users: share_with_users.as_ptr(),
            nb_users: share_with_users.len() as u32,
            share_with_groups: share_with_groups.as_ptr(),
            nb_groups: share_with_groups.len() as u32,
            share_with_self: self.share_with_self,
            padding_step: c_padding,
        };

        EncryptionOptionsWrapper::<'a> {
            _share_with_users: share_with_users,
            _share_with_groups: share_with_groups,
            c_options,
            phantom: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ctanker_version() {
        assert!(!super::CTankerLib::get().version_string().is_empty());
    }
}
