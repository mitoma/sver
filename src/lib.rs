mod filemode;

use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    path::{Path, PathBuf},
};

use self::filemode::SverFileMode;
use git2::{Oid, Repository};
use log::debug;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub fn list_sources(path: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let (repo, _work_dir, target_path) = resolve_target_repo_and_path(path)?;

    let entries = list_sorted_entries(&repo, &target_path)?;
    let result: Vec<String> = entries
        .iter()
        .map(|(path, _oid)| String::from_utf8(path.clone()).unwrap())
        .collect();
    Ok(result)
}

pub fn calc_version(path: &str) -> Result<Version, Box<dyn Error>> {
    let (repo, work_dir, target_path) = resolve_target_repo_and_path(path)?;

    let entries = list_sorted_entries(&repo, &target_path)?;
    let version = calc_hash_string(&repo, target_path.as_bytes(), &entries)?;

    let version = Version {
        repository_root: work_dir,
        path: target_path,
        version,
    };
    Ok(version)
}

fn resolve_target_repo_and_path(
    path: &str,
) -> Result<(Repository, String, String), Box<dyn Error>> {
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
    Ok((repo, work_dir, target_path))
}

pub struct Version {
    pub repository_root: String,
    pub path: String,
    pub version: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct SverConfig {
    #[serde(default)]
    excludes: Vec<String>,
    #[serde(default)]
    dependencies: Vec<String>,
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
    mode: SverFileMode,
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
        let containable = path_set.iter().any(|(include, excludes)| {
            let include_file =
                match_samefile_or_include_dir(entry.path.as_slice(), include.as_bytes());
            let exclude_file = excludes.iter().any(|exclude| {
                if include.is_empty() {
                    match_samefile_or_include_dir(entry.path.as_slice(), exclude.as_bytes())
                } else {
                    match_samefile_or_include_dir(
                        entry.path.as_slice(),
                        [include.as_bytes(), SEPARATOR, exclude.as_bytes()]
                            .concat()
                            .as_slice(),
                    )
                }
            });
            include_file && !exclude_file
        });
        debug!(
            "{:?}, containable:{}",
            String::from_utf8(entry.path.clone()),
            containable
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

const SEPARATOR: &[u8] = "/".as_bytes();
fn match_samefile_or_include_dir(test_path: &[u8], path: &[u8]) -> bool {
    let is_same_file = test_path == path;
    let is_contain_path =
        path.is_empty() || test_path.starts_with([path, SEPARATOR].concat().as_slice());
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

    if let Some(entry) = repo.index()?.get_path(p.as_path(), 0) {
        debug!("sver.toml exists. path:{:?}", String::from_utf8(entry.path));
        let sver_config: BTreeMap<String, SverConfig> =
            toml::from_slice(repo.find_blob(entry.id)?.content())?;
        let default_config = sver_config["default"].clone();
        path_and_excludes.insert(path.to_string(), default_config.excludes);

        for dependency_path in default_config.dependencies {
            collect_path_and_excludes(repo, &dependency_path, path_and_excludes)?;
        }
    } else {
        path_and_excludes.insert(path.to_string(), vec![]);
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
    use std::{
        env::temp_dir,
        fs::{create_dir_all, File},
        io::Write,
        path::Path,
        sync::Once,
    };

    use git2::{build::CheckoutBuilder, Commit, IndexAddOption, ObjectType, Repository, Signature};
    use log::debug;
    use uuid::Uuid;

    use crate::{calc_hash_string, list_sorted_entries};

    static INIT: Once = Once::new();

    pub fn initialize() {
        INIT.call_once(|| {
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

    fn add_file(repo: &Repository, path: String, content: &[u8]) {
        let workdir = repo.workdir().unwrap();
        let mut file_path = workdir.to_path_buf();
        file_path.push(&path);

        if let Some(parent_dir) = file_path.parent() {
            create_dir_all(parent_dir.to_str().unwrap()).unwrap();
        }
        let mut file = File::create(file_path).unwrap();
        file.write_all(content).unwrap();
    }

    fn find_last_commit(repo: &Repository) -> Result<Commit, git2::Error> {
        let obj = repo.head()?.resolve()?.peel(ObjectType::Commit)?;
        obj.into_commit()
            .map_err(|_| git2::Error::from_str("Couldn't find commit"))
    }

    fn add_and_commit(
        repo: &Repository,
        path: Option<&Path>,
        message: &str,
    ) -> Result<(), git2::Error> {
        let mut index = repo.index()?;
        if let Some(path) = path {
            index.add_path(path)?;
        } else {
            index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
        }

        let oid = index.write_tree()?;
        let signature = Signature::now("sver tester", "tester@example.com")?;
        let last_commit = find_last_commit(&repo).ok();

        let tree = repo.find_tree(oid)?;
        if let Some(parent_commit) = last_commit {
            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &[&parent_commit],
            )?
        } else {
            repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[])?
        };
        repo.checkout_head(Some(CheckoutBuilder::new().force()))
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
        add_file(&repo, "hello.txt".into(), "hello world!".as_bytes());
        add_file(
            &repo,
            "service1/world.txt".into(),
            "good morning!".as_bytes(),
        );
        add_and_commit(&repo, None, "setup").unwrap();
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
        assert_eq!("service1/world.txt".as_bytes(), key);

        assert_eq!(
            hash,
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
        add_file(
            &repo,
            "service1/hello.txt".into(),
            "hello world!".as_bytes(),
        );
        add_file(
            &repo,
            "service2/sver.toml".into(),
            "
        [default]
        dependencies = [
            \"service1\",
        ]"
            .as_bytes(),
        );
        add_and_commit(&repo, None, "setup").unwrap();
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
        add_file(
            &repo,
            "service1/sver.toml".into(),
            "
        [default]
        dependencies = [
            \"service2\",
        ]"
            .as_bytes(),
        );
        add_file(
            &repo,
            "service2/sver.toml".into(),
            "
        [default]
        dependencies = [
            \"service1\",
        ]"
            .as_bytes(),
        );
        add_and_commit(&repo, None, "setup").unwrap();

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
        add_file(&repo, "hello.txt".into(), "hello".as_bytes());
        add_file(
            &repo,
            "sver.toml".into(),
            "
        [default]
        excludes = [
            \"doc\",
        ]"
            .as_bytes(),
        );
        add_file(&repo, "doc/README.txt".into(), "README".as_bytes());
        add_and_commit(&repo, None, "setup").unwrap();
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
}
