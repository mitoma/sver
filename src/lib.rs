mod filemode;
mod sver_config;

use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    path::{Component, Path, PathBuf},
};

use self::filemode::FileMode;
use self::sver_config::SverConfig;
use git2::{Oid, Repository};
use log::debug;
use sha2::{Digest, Sha256};

pub fn init_sver_config(path: &str) -> Result<String, Box<dyn Error>> {
    debug!("path:{}", path);
    let ResolvePathResult {
        repo, target_path, ..
    } = resolve_target_repo_and_path(path)?;
    let mut path_buf = PathBuf::new();
    path_buf.push(target_path);
    path_buf.push("sver.toml");
    let config_path = path_buf.as_path();

    if repo.index()?.get_path(config_path, 0).is_some() {
        return Ok("sver.toml is already exists".into());
    }
    if !SverConfig::write_initial_config(config_path)? {
        return Ok(format!(
            "sver.toml is already exists. but not commited. path:{}",
            path
        ));
    }
    Ok(format!("sver.toml is generated. path:{}", path))
}

pub fn list_sources(path: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let ResolvePathResult {
        repo, target_path, ..
    } = resolve_target_repo_and_path(path)?;

    let entries = list_sorted_entries(&repo, &target_path)?;
    let result: Vec<String> = entries
        .iter()
        .map(|(path, _oid)| String::from_utf8(path.clone()).unwrap())
        .collect();
    Ok(result)
}

pub fn calc_version(path: &str) -> Result<Version, Box<dyn Error>> {
    let ResolvePathResult {
        repo,
        work_dir,
        target_path,
    } = resolve_target_repo_and_path(path)?;

    let entries = list_sorted_entries(&repo, &target_path)?;
    let version = calc_hash_string(&repo, target_path.as_bytes(), &entries)?;

    let version = Version {
        repository_root: work_dir,
        path: target_path,
        version,
    };
    Ok(version)
}

struct ResolvePathResult {
    repo: Repository,
    work_dir: String,
    target_path: String,
}

fn resolve_target_repo_and_path(path: &str) -> Result<ResolvePathResult, Box<dyn Error>> {
    let target_path = Path::new(path);
    let repo = find_repository(target_path)?;
    let target_path = relative_path(&repo, target_path)?;
    let target_path = target_path
        .iter()
        .flat_map(|os| os.to_str())
        .collect::<Vec<_>>()
        .join("/");
    let work_dir = repo
        .workdir()
        .and_then(|p| p.to_str())
        .ok_or("bare repository")?
        .to_string();
    debug!("repository_root:{}", work_dir);
    debug!("target_path:{}", target_path);
    Ok(ResolvePathResult {
        repo,
        work_dir,
        target_path,
    })
}

pub struct Version {
    pub repository_root: String,
    pub path: String,
    pub version: String,
}

fn relative_path(repo: &Repository, path: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let repo_path = repo
        .workdir()
        .and_then(|p| p.canonicalize().ok())
        .ok_or("bare repository is not supported")?;
    let current_path = path.canonicalize()?;
    let result = current_path.strip_prefix(repo_path)?.to_path_buf();
    Ok(result)
}

struct OidAndMode {
    oid: Oid,
    mode: FileMode,
}

fn calc_hash_string(
    repo: &Repository,
    target_path: &[u8],
    source: &BTreeMap<Vec<u8>, OidAndMode>,
) -> Result<String, Box<dyn Error>> {
    let mut hasher = Sha256::default();
    hasher.update(target_path);
    for (path, oid_and_mode) in source {
        hasher.update(path);
        match oid_and_mode.mode {
            FileMode::Blob | FileMode::Link => {
                // Q. Why little endian?
                // A. no reason.
                hasher.update(u32::from(oid_and_mode.mode).to_le_bytes());
                let blob = repo.find_blob(oid_and_mode.oid)?;
                let content = blob.content();
                hasher.update(content);
                debug!(
                    "path:{}, mode:{:?}, content:{}",
                    String::from_utf8(path.clone())?,
                    oid_and_mode.mode,
                    String::from_utf8(content.to_vec())?
                )
            }
            // Commit (Submodule の場合は参照先のコミットハッシュを計算対象に加える)
            FileMode::Commit => {
                debug!("commit_hash?:{}", oid_and_mode.oid);
                hasher.update(oid_and_mode.oid);
            }
            _ => {
                debug!(
                    "unsupported mode. skipped. path:{}, mode:{:?}",
                    String::from_utf8(path.clone())?,
                    oid_and_mode.mode
                )
            }
        }
    }
    let hash = format!("{:#x}", hasher.finalize());
    Ok(hash)
}

fn list_sorted_entries(
    repo: &Repository,
    path_str: &str,
) -> Result<BTreeMap<Vec<u8>, OidAndMode>, Box<dyn Error>> {
    let mut path_set: HashMap<String, Vec<String>> = HashMap::new();
    collect_path_and_excludes(repo, path_str, &mut path_set)?;
    debug!("dependency_paths:{:?}", path_set);
    let mut map = BTreeMap::new();
    for entry in repo.index()?.iter() {
        let containable = containable(entry.path.as_slice(), &path_set);
        debug!(
            "path:{}, containable:{}, mode:{:?}",
            String::from_utf8(entry.path.clone())?,
            containable,
            FileMode::from(entry.mode),
        );
        if containable {
            debug!("add path:{:?}", String::from_utf8(entry.path.clone()));
            map.insert(
                entry.path,
                OidAndMode {
                    oid: entry.id,
                    mode: entry.mode.into(),
                },
            );
        }
    }
    Ok(map)
}

#[cfg(target_os = "windows")]
const OS_SEP_STR: &str = "\\";
#[cfg(target_os = "linux")]
const OS_SEP_STR: &str = "/";

const SEPARATOR_STR: &str = "/";
const SEPARATOR_BYTE: &[u8] = SEPARATOR_STR.as_bytes();

fn containable(test_path: &[u8], path_set: &HashMap<String, Vec<String>>) -> bool {
    path_set.iter().any(|(include, excludes)| {
        let include_file = match_samefile_or_include_dir(test_path, include.as_bytes());
        let exclude_file = excludes.iter().any(|exclude| {
            if include.is_empty() {
                match_samefile_or_include_dir(test_path, exclude.as_bytes())
            } else {
                match_samefile_or_include_dir(
                    test_path,
                    [include.as_bytes(), SEPARATOR_BYTE, exclude.as_bytes()]
                        .concat()
                        .as_slice(),
                )
            }
        });
        include_file && !exclude_file
    })
}

fn match_samefile_or_include_dir(test_path: &[u8], path: &[u8]) -> bool {
    let is_same_file = test_path == path;
    let is_contain_path =
        path.is_empty() || test_path.starts_with([path, SEPARATOR_BYTE].concat().as_slice());
    is_same_file || is_contain_path
}

fn collect_path_and_excludes(
    repo: &Repository,
    path: &str,
    path_and_excludes: &mut HashMap<String, Vec<String>>,
) -> Result<(), Box<dyn Error>> {
    if path_and_excludes.contains_key(path) {
        debug!("already added. path:{}", path.to_string());
        return Ok(());
    }
    debug!("add dep path : {}", path);

    let mut p = PathBuf::new();
    p.push(path);
    p.push("sver.toml");

    let mut current_path_and_excludes: HashMap<String, Vec<String>> = HashMap::new();

    if let Some(entry) = repo.index()?.get_path(p.as_path(), 0) {
        debug!("sver.toml exists. path:{:?}", String::from_utf8(entry.path));
        let default_config =
            SverConfig::load_profile(repo.find_blob(entry.id)?.content(), "default")?;
        current_path_and_excludes.insert(path.to_string(), default_config.excludes.clone());
        path_and_excludes.insert(path.to_string(), default_config.excludes);
        for dependency_path in default_config.dependencies {
            collect_path_and_excludes(repo, &dependency_path, path_and_excludes)?;
        }
    } else {
        current_path_and_excludes.insert(path.to_string(), vec![]);
        path_and_excludes.insert(path.to_string(), vec![]);
    }

    // include synbolic link
    for entry in repo.index()?.iter() {
        let containable = containable(entry.path.as_slice(), &current_path_and_excludes);
        if containable && FileMode::Link == FileMode::from(entry.mode) {
            let path = String::from_utf8(entry.path)?;
            let mut buf = PathBuf::new();
            buf.push(path);
            buf.pop();

            let blob = repo.find_blob(entry.id)?;
            let link_path = String::from_utf8(blob.content().to_vec())?;
            let link_path = Path::new(&link_path);
            for link_components in link_path.components() {
                debug!("link_component:{:?}", link_components);
                match link_components {
                    Component::ParentDir => {
                        buf.pop();
                    }
                    Component::Normal(path) => buf.push(path),
                    Component::RootDir => {}
                    Component::CurDir => {}
                    Component::Prefix(_prefix) => {}
                }
            }

            let link_path = buf
                .to_str()
                .ok_or("path is invalid")?
                .replace(OS_SEP_STR, SEPARATOR_STR);
            debug!("collect link path. path:{}", &link_path);
            collect_path_and_excludes(repo, &link_path, path_and_excludes)?;
        }
    }
    Ok(())
}

fn find_repository(from_path: &Path) -> Result<Repository, Box<dyn Error>> {
    for target_path in from_path.canonicalize()?.ancestors() {
        if let Ok(repo) = Repository::open(target_path) {
            return Ok(repo);
        }
    }
    Err("repository was not found".into())
}

#[cfg(test)]
mod tests {
    use std::{env::temp_dir, path::Path, sync::Once};

    use git2::{Commit, IndexEntry, IndexTime, Oid, Repository, ResetType, Signature};
    use log::debug;
    use uuid::Uuid;

    use crate::{
        calc_hash_string, calc_version, filemode::FileMode, list_sorted_entries, list_sources,
    };

    static INIT: Once = Once::new();

    pub fn initialize() {
        INIT.call_once(|| {
            std::env::set_var("RUST_LOG", "debug");
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

    fn add_submodule(
        repo: &mut Repository,
        external_repo_url: &str,
        path: &str,
        commit_hash: &str,
    ) {
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

        // exercise
        let repo_path = repo.workdir().unwrap().to_str().unwrap();
        let sources = list_sources(repo_path).unwrap();
        let version = calc_version(repo_path).unwrap();

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
        let target_path = "service2";

        // exercise
        let entries = list_sorted_entries(&repo, target_path).unwrap();
        let hash = calc_hash_string(&repo, target_path.as_bytes(), &entries).unwrap();

        // verify
        assert_eq!(entries.len(), 2);

        let mut iter = entries.iter();
        let (key, _) = iter.next().unwrap();
        assert_eq!("service1/hello.txt".as_bytes(), key);
        let (key, _) = iter.next().unwrap();
        assert_eq!("service2/sver.toml".as_bytes(), key);

        assert_eq!(
            hash,
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
            let target_path = "service1";

            // exercise
            let entries = list_sorted_entries(&repo, target_path).unwrap();
            let hash = calc_hash_string(&repo, target_path.as_bytes(), &entries).unwrap();

            // verify
            assert_eq!(entries.len(), 2);

            let mut iter = entries.iter();
            let (key, _) = iter.next().unwrap();
            assert_eq!("service1/sver.toml".as_bytes(), key);
            let (key, _) = iter.next().unwrap();
            assert_eq!("service2/sver.toml".as_bytes(), key);

            assert_eq!(
                hash,
                "b3da97a449609fb4f3b14c47271b92858f2e4fa7986bfaa321a2a65ed775ae57"
            );
        }
        {
            let target_path = "service2";

            // exercise
            let entries = list_sorted_entries(&repo, target_path).unwrap();
            let hash = calc_hash_string(&repo, target_path.as_bytes(), &entries).unwrap();

            // verify
            assert_eq!(entries.len(), 2);

            let mut iter = entries.iter();
            let (key, _) = iter.next().unwrap();
            assert_eq!("service1/sver.toml".as_bytes(), key);
            let (key, _) = iter.next().unwrap();
            assert_eq!("service2/sver.toml".as_bytes(), key);

            assert_eq!(
                hash,
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
        let target_path = "";

        // exercise
        let entries = list_sorted_entries(&repo, target_path).unwrap();
        let hash = calc_hash_string(&repo, target_path.as_bytes(), &entries).unwrap();

        // verify
        assert_eq!(entries.len(), 2);

        let mut iter = entries.iter();
        let (key, _) = iter.next().unwrap();
        assert_eq!("hello.txt".as_bytes(), key);
        let (key, _) = iter.next().unwrap();
        assert_eq!("sver.toml".as_bytes(), key);

        assert_eq!(
            hash,
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
        let target_path = "";

        // exercise
        let entries = list_sorted_entries(&repo, target_path).unwrap();
        let hash = calc_hash_string(&repo, target_path.as_bytes(), &entries).unwrap();

        // verify
        assert_eq!(entries.len(), 2);

        let mut iter = entries.iter();
        let (key, _) = iter.next().unwrap();
        assert_eq!(".gitmodules".as_bytes(), key);
        let (key, id) = iter.next().unwrap();
        assert_eq!("bano".as_bytes(), key);
        assert_eq!(id.mode, FileMode::Commit);

        assert_eq!(
            hash,
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
        let target_path = "linkdir";

        // exercise
        let entries = list_sorted_entries(&repo, target_path).unwrap();
        let hash = calc_hash_string(&repo, target_path.as_bytes(), &entries).unwrap();

        // verify
        assert_eq!(entries.len(), 2);

        let mut iter = entries.iter();
        let (key, id) = iter.next().unwrap();
        assert_eq!("linkdir/symlink".as_bytes(), key);
        assert_eq!(id.mode, FileMode::Link);
        let (key, id) = iter.next().unwrap();
        assert_eq!("original/README.txt".as_bytes(), key);
        assert_eq!(id.mode, FileMode::Blob);

        assert_eq!(
            hash,
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
        let target_path = "linkdir";

        // exercise
        let entries = list_sorted_entries(&repo, target_path).unwrap();
        let hash = calc_hash_string(&repo, target_path.as_bytes(), &entries).unwrap();

        // verify
        assert_eq!(entries.len(), 3);

        let mut iter = entries.iter();
        let (key, id) = iter.next().unwrap();
        assert_eq!("linkdir/symlink".as_bytes(), key);
        assert_eq!(id.mode, FileMode::Link);
        let (key, id) = iter.next().unwrap();
        assert_eq!("original/README.txt".as_bytes(), key);
        assert_eq!(id.mode, FileMode::Blob);
        let (key, id) = iter.next().unwrap();
        assert_eq!("original/Sample.txt".as_bytes(), key);
        assert_eq!(id.mode, FileMode::Blob);

        assert_eq!(
            hash,
            "712093fffba02bcf58aefc2093064e6032183276940383b13145710ab2de7833"
        );
    }
}
