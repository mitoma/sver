// 本当は git2::FileMode を使いたかったが
// なぜか u32 → FileMode への変換を提供してくれていないので自前で用意する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileMode {
    Blob,
    BlobExecutable,
    Commit,
    Link,
    Tree,
    Unreadable,
    Unknown,
}

const GIT_FILEMODE_BLOB: u32 = libgit2_sys::GIT_FILEMODE_BLOB as u32;
const GIT_FILEMODE_BLOB_EXECUTABLE: u32 = libgit2_sys::GIT_FILEMODE_BLOB_EXECUTABLE as u32;
const GIT_FILEMODE_COMMIT: u32 = libgit2_sys::GIT_FILEMODE_COMMIT as u32;
const GIT_FILEMODE_LINK: u32 = libgit2_sys::GIT_FILEMODE_LINK as u32;
const GIT_FILEMODE_TREE: u32 = libgit2_sys::GIT_FILEMODE_TREE as u32;
const GIT_FILEMODE_UNREADABLE: u32 = libgit2_sys::GIT_FILEMODE_UNREADABLE as u32;

impl From<u32> for FileMode {
    fn from(value: u32) -> Self {
        match value {
            GIT_FILEMODE_BLOB => FileMode::Blob,
            GIT_FILEMODE_BLOB_EXECUTABLE => FileMode::BlobExecutable,
            GIT_FILEMODE_COMMIT => FileMode::Commit,
            GIT_FILEMODE_LINK => FileMode::Link,
            GIT_FILEMODE_TREE => FileMode::Tree,
            GIT_FILEMODE_UNREADABLE => FileMode::Unreadable,
            _ => FileMode::Unknown,
        }
    }
}

impl From<FileMode> for u32 {
    fn from(value: FileMode) -> Self {
        match value {
            FileMode::Blob => GIT_FILEMODE_BLOB,
            FileMode::BlobExecutable => GIT_FILEMODE_BLOB_EXECUTABLE,
            FileMode::Commit => GIT_FILEMODE_COMMIT,
            FileMode::Link => GIT_FILEMODE_LINK,
            FileMode::Tree => GIT_FILEMODE_TREE,
            FileMode::Unreadable => GIT_FILEMODE_UNREADABLE,
            FileMode::Unknown => GIT_FILEMODE_UNREADABLE,
        }
    }
}
