use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct VersionOutput {
    pub(crate) repository_root: String,
    pub(crate) path: String,
    pub(crate) version: String,
}

#[derive(Serialize)]
pub(crate) struct VersionsOutput {
    pub(crate) versions: Vec<VersionOutput>,
}

#[derive(Serialize)]
pub(crate) struct VersionFullOutput {
    repository_root: String,
    path: String,
    short_version: String,
    long_version: String,
}
