#[derive(Debug, Clone, Copy)]
pub(crate) enum SverFileMode {
    Blob,
    BlobExecutable,
    Commit,
    Link,
    Tree,
    Unreadable,
    Unknown,
}

impl From<u32> for SverFileMode {
    fn from(value: u32) -> Self {
        match value {
            libgit2_sys::GIT_FILEMODE_BLOB => SverFileMode::Blob,
            libgit2_sys::GIT_FILEMODE_BLOB_EXECUTABLE => SverFileMode::BlobExecutable,
            libgit2_sys::GIT_FILEMODE_COMMIT => SverFileMode::Commit,
            libgit2_sys::GIT_FILEMODE_LINK => SverFileMode::Link,
            libgit2_sys::GIT_FILEMODE_TREE => SverFileMode::Tree,
            libgit2_sys::GIT_FILEMODE_UNREADABLE => SverFileMode::Unreadable,
            _ => SverFileMode::Unknown,
        }
    }
}

impl From<SverFileMode> for u32 {
    fn from(value: SverFileMode) -> Self {
        match value {
            SverFileMode::Blob => libgit2_sys::GIT_FILEMODE_BLOB,
            SverFileMode::BlobExecutable => libgit2_sys::GIT_FILEMODE_BLOB_EXECUTABLE,
            SverFileMode::Commit => libgit2_sys::GIT_FILEMODE_COMMIT,
            SverFileMode::Link => libgit2_sys::GIT_FILEMODE_LINK,
            SverFileMode::Tree => libgit2_sys::GIT_FILEMODE_TREE,
            SverFileMode::Unreadable => libgit2_sys::GIT_FILEMODE_UNREADABLE,
            SverFileMode::Unknown => libgit2_sys::GIT_FILEMODE_UNREADABLE,
        }
    }
}
