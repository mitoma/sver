#[derive(Debug, Clone, Copy)]
pub(crate) enum FileMode {
    Blob,
    BlobExecutable,
    Commit,
    Link,
    Tree,
    Unreadable,
    Unknown,
}

impl From<u32> for FileMode {
    fn from(value: u32) -> Self {
        match value {
            libgit2_sys::GIT_FILEMODE_BLOB => FileMode::Blob,
            libgit2_sys::GIT_FILEMODE_BLOB_EXECUTABLE => FileMode::BlobExecutable,
            libgit2_sys::GIT_FILEMODE_COMMIT => FileMode::Commit,
            libgit2_sys::GIT_FILEMODE_LINK => FileMode::Link,
            libgit2_sys::GIT_FILEMODE_TREE => FileMode::Tree,
            libgit2_sys::GIT_FILEMODE_UNREADABLE => FileMode::Unreadable,
            _ => FileMode::Unknown,
        }
    }
}

impl From<FileMode> for u32 {
    fn from(value: FileMode) -> Self {
        match value {
            FileMode::Blob => libgit2_sys::GIT_FILEMODE_BLOB,
            FileMode::BlobExecutable => libgit2_sys::GIT_FILEMODE_BLOB_EXECUTABLE,
            FileMode::Commit => libgit2_sys::GIT_FILEMODE_COMMIT,
            FileMode::Link => libgit2_sys::GIT_FILEMODE_LINK,
            FileMode::Tree => libgit2_sys::GIT_FILEMODE_TREE,
            FileMode::Unreadable => libgit2_sys::GIT_FILEMODE_UNREADABLE,
            FileMode::Unknown => libgit2_sys::GIT_FILEMODE_UNREADABLE,
        }
    }
}
