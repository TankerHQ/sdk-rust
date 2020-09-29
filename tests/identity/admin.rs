use super::App;
use futures::executor::block_on;
use std::ffi::{CStr, CString};
use tankersdk::cadmin::{self, CAdmin};
use tankersdk::Error;

#[derive(Debug)]
pub struct Admin {
    cadmin: CAdmin,
    api_url: CString,
}

// SAFETY: cadmin functions are thread-safe
unsafe impl Send for Admin {}
unsafe impl Sync for Admin {}

impl Admin {
    pub async fn new(admin_url: &str, id_token: &str, api_url: &str) -> Result<Self, Error> {
        let admin_url = CString::new(admin_url).unwrap();
        let id_token = CString::new(id_token).unwrap();
        let api_url = CString::new(api_url).unwrap();
        let cadmin = cadmin::connect(&admin_url, &id_token).await?;
        Ok(Self { cadmin, api_url })
    }

    pub async fn create_app(&self, name: &str) -> Result<App, Error> {
        let cname = CString::new(name).unwrap();
        let descr = unsafe { cadmin::create_app(self.cadmin, &cname).await? };

        let id = unsafe { CStr::from_ptr((*descr).id) };
        let auth_token = unsafe { CStr::from_ptr((*descr).auth_token) };
        let private_key = unsafe { CStr::from_ptr((*descr).private_key) };

        let app = App {
            url: self.api_url.to_str().unwrap().to_string(),
            id: id.to_str().unwrap().to_string(),
            auth_token: auth_token.to_str().unwrap().to_string(),
            private_key: private_key.to_str().unwrap().to_string(),
        };

        // SAFETY: This comes straight from create_app, free'd at most once. Panics are OK.
        unsafe { cadmin::descriptor_free(descr) };

        Ok(app)
    }

    pub async fn delete_app(&self, id: &str) -> Result<(), Error> {
        let cid = CString::new(id).unwrap();
        unsafe { cadmin::delete_app(self.cadmin, &cid).await }
    }

    pub async fn app_update(
        &self,
        id: &str,
        oidc_client_id: &str,
        oidc_provider: &str,
    ) -> Result<(), Error> {
        let cid = CString::new(id).unwrap();
        let coidc_client_id = CString::new(oidc_client_id).unwrap();
        let coidc_provider = CString::new(oidc_provider).unwrap();
        unsafe { cadmin::app_update(self.cadmin, &cid, &coidc_client_id, &coidc_provider).await }
    }
}

impl Drop for Admin {
    fn drop(&mut self) {
        // NOTE: Ignore errors, nothing we can do
        let _ = block_on(unsafe { cadmin::destroy(self.cadmin) });
    }
}
