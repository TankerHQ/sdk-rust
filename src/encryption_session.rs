use crate::ctanker::{self, CEncSessPtr};
use crate::Error;
use futures::executor::block_on;
use futures::AsyncRead;

/// Allows performing multiple encryption operations with a reduced number of keys.
///
/// See https://docs.tanker.io/latest/guides/encryption-session/ for a detailed guide.
pub struct EncryptionSession {
    csess: CEncSessPtr,
}

impl EncryptionSession {
    pub(crate) unsafe fn new(csess: CEncSessPtr) -> Self {
        Self { csess }
    }

    /// Encrypt some data as part of the encryption session.
    pub async fn encrypt<T: AsRef<[u8]>>(&self, data: T) -> Result<Vec<u8>, Error> {
        unsafe { ctanker::encryption_session_encrypt(self.csess, data.as_ref()).await }
    }

    /// Creates an encryption stream bound to this encryption session wrapping `data`.
    pub async fn encrypt_stream<UserStream: AsyncRead + Unpin>(
        &self,
        data: UserStream,
    ) -> Result<impl AsyncRead + Unpin, Error> {
        unsafe { ctanker::encryption_session_encrypt_stream(self.csess, data).await }
    }

    /// Get the resource ID of this encryption session that can be used to call [share](crate::Core::share),
    /// which will share all the data encrypted within this session.
    pub fn get_resource_id(&self) -> String {
        block_on(ctanker::encryption_session_get_resource_id(self.csess))
    }
}

impl Drop for EncryptionSession {
    fn drop(&mut self) {
        // Ignore errors, nothing we can do
        let _ = unsafe { block_on(ctanker::encryption_session_close(self.csess)) };
    }
}
