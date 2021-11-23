use std::ffi::CString;

/// The `share_with_users` and `share_with_groups` methods allow you to specify who will be able to decrypt the resource.
/// By default the resource will be shared with its creator. To prevent that, call `share_with_self` with `false`.
///
/// In general, if you need to share a resource with multiple users, it is advised to create groups and use `share_with_groups`.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct EncryptionOptions {
    pub(crate) share_with_users: Vec<CString>,
    pub(crate) share_with_groups: Vec<CString>,
    pub(crate) share_with_self: bool,
}

impl EncryptionOptions {
    pub fn new() -> Self {
        Default::default()
    }

    /// Sets the user public Tanker identities to share with
    pub fn share_with_users<S: AsRef<str>, Iter: IntoIterator<Item = S>>(
        mut self,
        users: Iter,
    ) -> Self {
        self.share_with_users = users
            .into_iter()
            .map(|u| CString::new(u.as_ref()).unwrap())
            .collect();
        self
    }

    /// Sets the Group IDs to share with
    pub fn share_with_groups<S: AsRef<str>, Iter: IntoIterator<Item = S>>(
        mut self,
        groups: Iter,
    ) -> Self {
        self.share_with_groups = groups
            .into_iter()
            .map(|g| CString::new(g.as_ref()).unwrap())
            .collect();
        self
    }

    /// Sets whether the encrypted data should be decryptable by the author
    pub fn share_with_self(mut self, share_with_self: bool) -> Self {
        self.share_with_self = share_with_self;
        self
    }
}

impl Default for EncryptionOptions {
    fn default() -> Self {
        Self {
            share_with_users: vec![],
            share_with_groups: vec![],
            share_with_self: true,
        }
    }
}

/// The `share_with_users` and `share_with_groups` methods allow you to specify who will be able to decrypt the resource.
///
/// In general, if you need to share a resource with multiple users, it is advised to create groups and use `share_with_groups`.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct SharingOptions {
    pub(crate) share_with_users: Vec<CString>,
    pub(crate) share_with_groups: Vec<CString>,
}

impl SharingOptions {
    pub fn new() -> Self {
        Default::default()
    }

    /// Sets the user public Tanker identities to share with
    pub fn share_with_users<S: AsRef<str>, Iter: IntoIterator<Item = S>>(
        mut self,
        users: Iter,
    ) -> Self {
        self.share_with_users = users
            .into_iter()
            .map(|u| CString::new(u.as_ref()).unwrap())
            .collect();
        self
    }

    /// Sets the Group IDs to share with
    pub fn share_with_groups<S: AsRef<str>, Iter: IntoIterator<Item = S>>(
        mut self,
        groups: Iter,
    ) -> Self {
        self.share_with_groups = groups
            .into_iter()
            .map(|g| CString::new(g.as_ref()).unwrap())
            .collect();
        self
    }
}
