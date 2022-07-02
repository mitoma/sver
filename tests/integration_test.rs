use std::{env::temp_dir, path::Path, sync::Once};

use git2::{Commit, IndexEntry, IndexTime, Oid, Repository, ResetType, Signature};
use log::debug;
use uuid::Uuid;

use sver::{calc_version, filemode::FileMode, list_sources};

static INIT: Once = Once::new();

pub fn initialize() {
    INIT.call_once(|| {
        //std::env::set_var("RUST_LOG", "debug");
        env_logger::init();
    });
}

// テスト用リポジトリを作る
fn setup_test_repository() -> Repository {
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

fn add_blog(repo: &Repository, path: &str, content: &[u8]) {
    add_file(repo, path, content, FileMode::Blob)
}

fn add_symlink(repo: &Repository, link: &str, original: &str) {
    add_file(repo, link, original.as_bytes(), FileMode::Link)
}

fn add_submodule(repo: &mut Repository, external_repo_url: &str, path: &str, commit_hash: &str) {
    let path_obj = Path::new(path);
    let mut submodule = repo.submodule(&external_repo_url, &path_obj, true).unwrap();
    submodule.clone(None).unwrap();
    submodule.add_finalize().unwrap();
    let submodule_repo = submodule.open().unwrap();
    submodule_repo
        .set_head_detached(Oid::from_str(commit_hash).unwrap())
        .unwrap();
}

fn commit(repo: &Repository, commit_message: &str) {
    let id = repo.index().unwrap().write_tree().unwrap();
    let tree = repo.find_tree(id).unwrap();
    let signature = Signature::now("sver tester", "tester@example.com").unwrap();
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
    let obj = repo.find_object(commit, None).unwrap();
    repo.reset(&obj, ResetType::Hard, None).unwrap();
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

fn calc_target_path(repo: &Repository, path: &str) -> String {
    let mut path_buf = repo.workdir().unwrap().to_path_buf();
    path_buf.push(path);
    path_buf.to_str().unwrap().into()
}

// repo layout
// .
// + hello.txt
// + service1/world.txt
#[test]
fn simple_repository() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blog(&repo, "hello.txt", "hello world!".as_bytes());
    add_blog(&repo, "service1/world.txt", "good morning!".as_bytes());
    commit(&repo, "setup");

    let target_path = &calc_target_path(&repo, "");

    // exercise
    let sources = list_sources(target_path).unwrap();
    let version = calc_version(target_path).unwrap();

    // verify
    assert_eq!(sources, vec!["hello.txt", "service1/world.txt"]);
    assert_eq!(
        version.version,
        "c7eacf9aee8ced0b9131dce96c2e2077e2c683a7d39342c8c13b32fefac5662a"
    );
}

// repo layout
// .
// + service1/hello.txt
// + service2/sver.toml → dependency = [ "service1" ]
#[test]
fn has_dependencies_repository() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blog(&repo, "service1/hello.txt", "hello world!".as_bytes());
    add_blog(
        &repo,
        "service2/sver.toml",
        "
        [default]
        dependencies = [
            \"service1\",
        ]"
        .as_bytes(),
    );
    commit(&repo, "setup");

    let target_path = &calc_target_path(&repo, "service2");

    // exercise
    let sources = list_sources(target_path).unwrap();
    let version = calc_version(target_path).unwrap();

    // verify
    assert_eq!(sources, vec!["service1/hello.txt", "service2/sver.toml"]);
    assert_eq!(
        version.version,
        "0cb6c0434a87e4ce7f18388365004a4809664cfd2c86b6bbd2b1572a005a564a"
    );
}

// repo layout
// .
// + service1/sver.toml → dependency = [ "service2" ]
// + service2/sver.toml → dependency = [ "service1" ]
#[test]
fn cyclic_repository() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blog(
        &repo,
        "service1/sver.toml",
        "
        [default]
        dependencies = [
            \"service2\",
        ]"
        .as_bytes(),
    );
    add_blog(
        &repo,
        "service2/sver.toml",
        "
        [default]
        dependencies = [
            \"service1\",
        ]"
        .as_bytes(),
    );
    commit(&repo, "setup");

    {
        let target_path = &calc_target_path(&repo, "service1");

        // exercise
        let sources = list_sources(target_path).unwrap();
        let version = calc_version(target_path).unwrap();

        // verify
        assert_eq!(sources, vec!["service1/sver.toml", "service2/sver.toml"]);
        assert_eq!(
            version.version,
            "b3da97a449609fb4f3b14c47271b92858f2e4fa7986bfaa321a2a65ed775ae57"
        );
    }
    {
        let target_path = &calc_target_path(&repo, "service2");

        // exercise
        let sources = list_sources(target_path).unwrap();
        let version = calc_version(target_path).unwrap();

        // verify
        assert_eq!(sources, vec!["service1/sver.toml", "service2/sver.toml"]);
        assert_eq!(
            version.version,
            "d48299e3ecbd6943a51042d436002f06086c7b4d9d50bd1e2ad6d872bd4fb3d7"
        );
    }
}

// repo layout
// .
// + hello.txt
// + sver.toml → excludes = [ "doc" ]
// + doc
//   + README.txt
#[test]
fn has_exclude_repository() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blog(&repo, "hello.txt", "hello".as_bytes());
    add_blog(
        &repo,
        "sver.toml",
        "
        [default]
        excludes = [
            \"doc\",
        ]"
        .as_bytes(),
    );
    add_blog(&repo, "doc/README.txt", "README".as_bytes());
    commit(&repo, "setup");

    let target_path = &calc_target_path(&repo, "");

    // exercise
    let sources = list_sources(target_path).unwrap();
    let version = calc_version(target_path).unwrap();

    // verify
    assert_eq!(sources, vec!["hello.txt", "sver.toml"]);
    assert_eq!(
        version.version,
        "a53b015257360d95600b8f0b749c01a651e803aa05395a8f6b39e194f95c3dfe"
    );
}

// repo layout
// .
// + bano → submodule https://github.com/mitoma/bano ec3774f3ad6abb46344cab9662a569a2f8231642
#[test]
fn has_submodule() {
    initialize();

    // setup
    let mut repo = setup_test_repository();
    add_submodule(
        &mut repo,
        "https://github.com/mitoma/bano",
        "bano",
        "ec3774f3ad6abb46344cab9662a569a2f8231642",
    );

    commit(&repo, "setup");

    let target_path = &calc_target_path(&repo, "");

    // exercise
    let sources = list_sources(target_path).unwrap();
    let version = calc_version(target_path).unwrap();

    // verify
    assert_eq!(sources, vec![".gitmodules", "bano"]);
    assert_eq!(
        version.version,
        "2600f60368549f186d7b48fe48765dbd57580cc416e91dc3fbca264d62d18f31"
    );
}

// repo layout
// .
// + linkdir
//   + symlink → original/README.txt
// + original
//   + README.txt
#[test]
fn has_symlink_single() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blog(&repo, "original/README.txt", "hello.world".as_bytes());
    add_symlink(&repo, "linkdir/symlink", "../original/README.txt");
    commit(&repo, "setup");

    let target_path = &calc_target_path(&repo, "linkdir");

    // exercise
    let sources = list_sources(target_path).unwrap();
    let version = calc_version(target_path).unwrap();

    // verify
    assert_eq!(sources, vec!["linkdir/symlink", "original/README.txt"]);
    assert_eq!(
        version.version,
        "604b932c22dc969de21c8241ff46ea40f1a37d36050cc9d11345679389552d29"
    );
}

// repo layout
// .
// + linkdir
//   + symlink → original/README.txt
// + original
//   + README.txt
//   + Sample.txt
#[test]
fn has_symlink_dir() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blog(&repo, "original/README.txt", "hello.world".as_bytes());
    add_blog(&repo, "original/Sample.txt", "sample".as_bytes());

    add_symlink(&repo, "linkdir/symlink", "../original");
    commit(&repo, "setup");

    let target_path = &calc_target_path(&repo, "linkdir");

    // exercise
    let sources = list_sources(target_path).unwrap();
    let version = calc_version(target_path).unwrap();

    // verify
    assert_eq!(
        sources,
        vec![
            "linkdir/symlink",
            "original/README.txt",
            "original/Sample.txt"
        ]
    );
    assert_eq!(
        version.version,
        "712093fffba02bcf58aefc2093064e6032183276940383b13145710ab2de7833"
    );
}
