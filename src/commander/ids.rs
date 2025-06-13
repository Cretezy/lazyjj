/*!
Helper structs [ChangeId] and [CommitId]
*/
use std::{ffi::OsStr, fmt::Display};

/// Wrapper around change ID.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ChangeId(pub String);

impl ChangeId {
    pub fn as_os_str(&self) -> &OsStr {
        OsStr::new(&self.0)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn as_string(&self) -> String {
        self.0.to_owned()
    }
}

impl AsRef<OsStr> for ChangeId {
    fn as_ref(&self) -> &OsStr {
        self.as_os_str()
    }
}

impl Display for ChangeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Wrapper around commit ID.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct CommitId(pub String);

impl CommitId {
    pub fn as_os_str(&self) -> &OsStr {
        OsStr::new(&self.0)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    // pub fn as_string(&self) -> String {
    //     self.0.to_owned()
    // }
}

impl AsRef<OsStr> for CommitId {
    fn as_ref(&self) -> &OsStr {
        self.as_os_str()
    }
}

impl Display for CommitId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
