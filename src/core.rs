use crate::ctanker;
use crate::*;
use futures::executor::block_on;
use futures::AsyncRead;
use std::ffi::CString;

#[derive(Debug)]
#[non_exhaustive]
pub struct Core {
    ctanker: ctanker::CTankerPtr,
}

impl Core {
    /// Creates a Tanker Core session with [Status](enum.Status.html) `Stopped`.
    ///
    /// ```no_run
    /// # use tankersdk::*;
    /// # async {
    /// let app_id = "Your tanker App ID".to_string();
    /// let writable_path = "/some/writable/path".to_string();
    /// let tanker = Core::new(Options::new(app_id, writable_path)).await?;
    /// # Result::<(), Error>::Ok(()) };
    /// ```
    pub async fn new(options: Options) -> Result<Self, Error> {
        ctanker::init();
        let ctanker = ctanker::create(options).await?;

        Ok(Self { ctanker })
    }

    pub fn set_log_handler(callback: Box<dyn Fn(LogRecord) + Send>) {
        unsafe { ctanker::set_log_handler(callback) }
    }

    /// The version of the Rust SDK crate
    pub fn version() -> &'static str {
        const VERSION: &str = env!("CARGO_PKG_VERSION");
        VERSION
    }

    /// The version of the native SDK
    #[doc(hidden)]
    pub fn native_version() -> &'static str {
        ctanker::version_string()
    }

    /// The status of the Tanker session
    pub fn status(&self) -> Status {
        unsafe { ctanker::status(self.ctanker) }
    }

    /// The current device's ID.
    ///
    /// The Tanker status must be `Ready`.
    pub fn device_id(&self) -> Result<String, Error> {
        unsafe { block_on(ctanker::device_id(self.ctanker)) }
    }

    /// The user's devices list.
    ///
    /// The Tanker status must be `Ready`.
    pub async fn device_list(&self) -> Result<Vec<Device>, Error> {
        unsafe { ctanker::device_list(self.ctanker).await }
    }

    /// Starts a Tanker session and returns a [Status](enum.Status.html).
    ///
    /// The status before calling this function must be `Stopped`.
    ///
    /// # Arguments
    /// * `identity` - A Tanker identity to use for this session
    pub async fn start(&self, identity: &str) -> Result<Status, Error> {
        unsafe { ctanker::start(self.ctanker, identity).await }
    }

    /// Stops the current Tanker session.
    pub async fn stop(&self) -> Result<(), Error> {
        unsafe { ctanker::stop(self.ctanker).await }
    }

    /// Registers the user's identity with which [start()](Self::start) has been called, and starts the session.
    ///
    /// The Tanker status must be `IdentityRegistrationNeeded`.
    ///
    /// # Arguments
    /// * `verification` - The verification to use for identity registration
    pub async fn register_identity(
        &self,
        verification: &Verification,
        options: &VerificationOptions,
    ) -> Result<Option<String>, Error> {
        let verif_wrapper = verification.to_cverification_wrapper();
        unsafe {
            ctanker::register_identity(self.ctanker, verif_wrapper.as_cverification(), options)
                .await
        }
    }

    /// Verifies the user's identity with which [start()](Self::start) has been called, and starts the session.
    ///
    /// The Tanker status must be `IdentityVerificationNeeded`.
    ///
    /// # Arguments
    /// * `verification` - The verification to use
    pub async fn verify_identity(
        &self,
        verification: &Verification,
        options: &VerificationOptions,
    ) -> Result<Option<String>, Error> {
        let verif_wrapper = verification.to_cverification_wrapper();
        unsafe {
            ctanker::verify_identity(self.ctanker, verif_wrapper.as_cverification(), options).await
        }
    }

    /// Attaches a provisional identity to the user.
    ///
    /// The Tanker status must be `Ready`.
    ///
    /// Depending on the result, verifying the provisional identity with [verify_provisional_identity](Self::verify_provisional_identity) might be necessary.
    pub async fn attach_provisional_identity(&self, identity: &str) -> Result<AttachResult, Error> {
        let identity = CString::new(identity).unwrap();
        unsafe { ctanker::attach_provisional_identity(self.ctanker, &identity).await }
    }

    /// Verifies an attached provisional identity.
    ///
    /// To be called when the status returned by [attach_provisional_identity](Self::attach_provisional_identity) is `IdentityVerificationNeeded`.
    ///
    /// Once the provisional identity is verified, every resource shared with it can now be decrypted by the user, who also joins every group in which the provisional identity was a member.
    pub async fn verify_provisional_identity(&self, prov: &Verification) -> Result<(), Error> {
        let verif_wrapper = prov.to_cverification_wrapper();
        unsafe {
            ctanker::verify_provisional_identity(self.ctanker, verif_wrapper.as_cverification())
                .await
        }
    }

    /// Adds or overrides a verification method.
    ///
    /// The Tanker status must be `Ready`.
    ///
    /// # Arguments
    /// * `verification` - The verification to set
    pub async fn set_verification_method(
        &self,
        verification: &Verification,
        options: &VerificationOptions,
    ) -> Result<Option<String>, Error> {
        let verif_wrapper = verification.to_cverification_wrapper();
        unsafe {
            ctanker::set_verification_method(
                self.ctanker,
                verif_wrapper.as_cverification(),
                options,
            )
            .await
        }
    }

    /// Returns the list of registered verification methods.
    ///
    /// The Tanker status must be either `IdentityVerificationNeeded` or `Ready`.
    pub async fn get_verification_methods(&self) -> Result<Vec<VerificationMethod>, Error> {
        unsafe { ctanker::get_verification_methods(self.ctanker).await }
    }

    /// Generates a verification key and returns its private part, which is required to verify the user's identity.
    ///
    /// The Tanker status must be `IdentityRegistrationNeeded`.
    ///
    /// Once the verification key has been registered, it is not possible to set up high-level verification methods (e.g. email/passphrase).
    ///
    /// **Warning**: This is a low level function for specific use-cases only, as it can have severe security implications.
    ///             Use it only if high-level identity verification doesn't fit your needs, and you fully understand how it works. Don't hesitate to contact Tanker for help.
    pub async fn generate_verification_key(&self) -> Result<String, Error> {
        unsafe { ctanker::generate_verification_key(self.ctanker).await }
    }

    /// Encrypts data and returns the resulting encrypted resource. It will be shared with individual users and groups specified in the [EncryptionOptions](EncryptionOptions).
    ///
    /// The Tanker status must be `Ready`.
    ///
    /// # Arguments
    /// * `data` - The clear data to encrypt
    /// * `options` - Encryption and sharing options
    pub async fn encrypt<T: AsRef<[u8]>>(
        &self,
        data: T,
        options: &EncryptionOptions,
    ) -> Result<Vec<u8>, Error> {
        unsafe { ctanker::encrypt(self.ctanker, data.as_ref(), options).await }
    }

    /// Decrypts a resource and returns the clear data.
    pub async fn decrypt<T: AsRef<[u8]>>(&self, data: T) -> Result<Vec<u8>, Error> {
        unsafe { ctanker::decrypt(self.ctanker, data.as_ref()).await }
    }

    /// Creates an encryption stream wrapping `data`.
    ///
    /// The Tanker status must be `Ready`.
    ///
    /// # Arguments
    /// * `data` - The stream containing data to encrypt
    /// * `options` - Encryption and sharing options
    pub async fn encrypt_stream<UserStream: AsyncRead + Unpin>(
        &self,
        data: UserStream,
        options: &EncryptionOptions,
    ) -> Result<impl AsyncRead + Unpin, Error> {
        unsafe { ctanker::encrypt_stream(self.ctanker, data, options).await }
    }

    /// Creates a decryption stream wrapping `data`.
    pub async fn decrypt_stream<UserStream: AsyncRead + Unpin>(
        &self,
        data: UserStream,
    ) -> Result<impl AsyncRead + Unpin, Error> {
        unsafe { ctanker::decrypt_stream(self.ctanker, data).await }
    }

    /// Retrieves an encrypted resource's ID.
    /// The resource ID can then be used to call [share](Self::share).
    pub fn get_resource_id(&self, data: &[u8]) -> Result<String, Error> {
        block_on(ctanker::get_resource_id(data))
    }

    /// Shares resources with users and groups.
    ///
    /// The Tanker status must be `Ready`.
    ///
    /// This function either fully succeeds or fails. In this case, it does not share with any recipient.
    ///
    /// # Arguments
    /// `resource_ids` - Resource IDs to share
    /// `options` - Defines the recipients of the sharing operation
    pub async fn share<S, Iter>(
        &self,
        resource_ids: Iter,
        options: &SharingOptions,
    ) -> Result<(), Error>
    where
        S: AsRef<str>,
        Iter: IntoIterator<Item = S>,
    {
        let resource_ids: Vec<_> = resource_ids
            .into_iter()
            .map(|r| CString::new(r.as_ref()).unwrap())
            .collect();
        unsafe { ctanker::share(self.ctanker, &resource_ids, options).await }
    }

    /// Creates a group with users' public identities, and returns its ID.
    ///
    /// The Tanker status must be `Ready`.
    ///
    /// **Note**: The maximum number of users per group is 1000.
    pub async fn create_group<S, Iter>(&self, member_identities: Iter) -> Result<String, Error>
    where
        S: AsRef<str>,
        Iter: IntoIterator<Item = S>,
    {
        let members: Vec<_> = member_identities
            .into_iter()
            .map(|r| CString::new(r.as_ref()).unwrap())
            .collect();
        unsafe { ctanker::create_group(self.ctanker, &members).await }
    }

    /// Add or remove members to an existing group.
    ///
    /// The Tanker status must be `Ready`.
    ///
    /// The new group members will automatically get access to all resources previously shared with the group.
    ///
    /// # Arguments
    /// * `group_id` - Group ID to modify
    /// * `users_to_add` - Public identities of users to add to the group
    pub async fn update_group_members<S, AddIter, RemoveIter>(
        &self,
        group_id: &str,
        users_to_add: AddIter,
        users_to_remove: RemoveIter,
    ) -> Result<(), Error>
    where
        S: AsRef<str>,
        AddIter: IntoIterator<Item = S>,
        RemoveIter: IntoIterator<Item = S>,
    {
        let group_id = CString::new(group_id).unwrap();
        let users_to_add: Vec<_> = users_to_add
            .into_iter()
            .map(|r| CString::new(r.as_ref()).unwrap())
            .collect();
        let users_to_remove: Vec<_> = users_to_remove
            .into_iter()
            .map(|r| CString::new(r.as_ref()).unwrap())
            .collect();
        unsafe { ctanker::update_group_members(self.ctanker, &group_id, &users_to_add, &users_to_remove).await }
    }

    /// Revokes one of the user's devices.
    ///
    /// The Tanker status must be `Ready`.
    #[deprecated(since = "2.8.0")]
    pub async fn revoke_device(&self, device_id: &str) -> Result<(), Error> {
        unsafe { ctanker::revoke_device(self.ctanker, device_id).await }
    }

    /// Create an encryption session that will allow performing multiple encryption operations with a reduced number of keys.
    ///
    /// # Arguments
    /// * `options` - Users and/or groups with whom to share the content encrypted in the session
    pub async fn create_encryption_session(
        &self,
        options: &EncryptionOptions,
    ) -> Result<EncryptionSession, Error> {
        let ptr = unsafe { ctanker::encryption_session_open(self.ctanker, options).await? };
        Ok(unsafe { EncryptionSession::new(ptr) })
    }

    /// Utility function to hash a password client side.
    ///
    /// This function is only useful in the specific case described in the [verification by passphrase guide](https://docs.tanker.io/latest/guides/verification-passphrase#using_the_application_password_as_a_passphrase).
    pub fn prehash_password(password: &str) -> Result<String, Error> {
        block_on(ctanker::prehash_password(password))
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        block_on(unsafe { ctanker::destroy(self.ctanker) });
    }
}
