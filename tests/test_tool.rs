use std::{env::temp_dir, path::Path, sync::Once};

use chrono::{DateTime, Utc};
use git2::{Commit, IndexEntry, IndexTime, Oid, Repository, ResetType, Signature, Time};
use log::debug;
use uuid::Uuid;

use sver::filemode::FileMode;

static INIT: Once = Once::new();

pub fn initialize() {
    INIT.call_once(|| {
        //std::env::set_var("RUST_LOG", "debug");
        env_logger::init();
    });
}

// テスト用リポジトリを作る
pub fn setup_test_repository() -> Repository {
    let mut tmp_dir = temp_dir();
    let uuid = Uuid::new_v4();
    tmp_dir.push(format!("sver-{}", uuid.to_string()));

    let repository_path = tmp_dir.as_path();

    let repo = Repository::init(repository_path).unwrap();
    debug!("{:?}", tmp_dir);
    repo
}

fn add_file(repo: &Repository, path: &str, content: &[u8], mode: FileMode) {
    let mut index = repo.index().unwrap();

    let blob = repo.blob(content).unwrap();
    let mut entry = entry();
    entry.mode = mode.into();
    entry.id = blob;
    entry.path = path.as_bytes().to_vec();
    index.add(&entry).unwrap();
    index.write().unwrap();
}

pub fn add_blob(repo: &Repository, path: &str, content: &[u8]) {
    add_file(repo, path, content, FileMode::Blob)
}

pub fn add_blob_executable(repo: &Repository, path: &str, content: &[u8]) {
    add_file(repo, path, content, FileMode::BlobExecutable)
}

pub fn add_symlink(repo: &Repository, link: &str, original: &str) {
    add_file(repo, link, original.as_bytes(), FileMode::Link)
}

pub fn add_submodule(
    repo: &mut Repository,
    external_repo_url: &str,
    path: &str,
    commit_hash: &str,
) {
    let mut index = repo.index().unwrap();
    let path_obj = Path::new(path);
    let mut submodule = repo.submodule(&external_repo_url, &path_obj, true).unwrap();
    submodule.clone(None).unwrap();
    submodule.add_finalize().unwrap();
    let submodule_repo = submodule.open().unwrap();
    submodule_repo
        .set_head_detached(Oid::from_str(commit_hash).unwrap())
        .unwrap();
    index.add_path(Path::new(path)).unwrap();
    index.write().unwrap();
}

pub fn commit_at(repo: &Repository, commit_message: &str, time: DateTime<Utc>) {
    let time = Time::new(time.timestamp_millis(), 0);

    let id = repo.index().unwrap().write_tree().unwrap();
    let tree = repo.find_tree(id).unwrap();
    let signature = Signature::new("sver tester", "tester@example.com", &time).unwrap();
    let mut parents = Vec::new();
    if let Ok(parent_id) = repo.refname_to_id("HEAD") {
        let parent_commit = repo.find_commit(parent_id).unwrap();
        parents.push(parent_commit);
    }

    let commit = repo
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            commit_message,
            &tree,
            parents.iter().collect::<Vec<&Commit>>().as_slice(),
        )
        .unwrap();
    debug!("commit hash:{:?}", commit);
    let obj = repo.find_object(commit, None).unwrap();
    repo.reset(&obj, ResetType::Hard, None).unwrap();
}

pub fn commit(repo: &Repository, commit_message: &str) {
    commit_at(repo, commit_message, Utc::now());
}

fn entry() -> IndexEntry {
    IndexEntry {
        ctime: IndexTime::new(0, 0),
        mtime: IndexTime::new(0, 0),
        dev: 0,
        ino: 0,
        mode: 0o100644,
        uid: 0,
        gid: 0,
        file_size: 0,
        id: Oid::from_bytes(&[0; 20]).unwrap(),
        flags: 0,
        flags_extended: 0,
        path: Vec::new(),
    }
}

pub fn calc_target_path(repo: &Repository, path: &str) -> String {
    let mut path_buf = repo.workdir().unwrap().to_path_buf();
    path_buf.push(path);
    path_buf.to_str().unwrap().into()
}

pub fn calc_target_path_with_profile(repo: &Repository, path: &str, profile: &str) -> String {
    let mut path_buf = repo.workdir().unwrap().to_path_buf();
    path_buf.push(path);
    format!("{}:{}", path_buf.to_str().unwrap(), profile)
}
