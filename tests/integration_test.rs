mod test_tool;

use sver::{sver_config::ValidationResult, sver_repository::SverRepository};

use crate::test_tool::{
    add_blog, add_blog_executable, add_submodule, add_symlink, calc_target_path,
    calc_target_path_with_profile, commit, initialize, setup_test_repository,
};

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

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "")).unwrap();

    // exercise
    let sources = sver_repo.list_sources().unwrap();
    let version = sver_repo.calc_version().unwrap();

    // verify
    assert_eq!(sources, vec!["hello.txt", "service1/world.txt"]);
    assert_eq!(
        version.version,
        "c7eacf9aee8ced0b9131dce96c2e2077e2c683a7d39342c8c13b32fefac5662a"
    );
}

// repo layout
// .
// + hello.txt (executable)
// + service1/world.txt
#[test]
fn has_blob_executable() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blog_executable(&repo, "hello.txt", "hello world!".as_bytes());
    add_blog(&repo, "service1/world.txt", "good morning!".as_bytes());
    commit(&repo, "setup");

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "")).unwrap();

    // exercise
    let sources = sver_repo.list_sources().unwrap();
    let version = sver_repo.calc_version().unwrap();

    // verify
    assert_eq!(sources, vec!["hello.txt", "service1/world.txt"]);
    assert_eq!(
        version.version,
        "435f0baae5406a75a66e515bf1674db348382139b8443a695a2b1c2925935160"
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

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "service2")).unwrap();

    // exercise
    let sources = sver_repo.list_sources().unwrap();
    let version = sver_repo.calc_version().unwrap();

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
        let sver_repo = SverRepository::new(&calc_target_path(&repo, "service1")).unwrap();

        // exercise
        let sources = sver_repo.list_sources().unwrap();
        let version = sver_repo.calc_version().unwrap();

        // verify
        assert_eq!(sources, vec!["service1/sver.toml", "service2/sver.toml"]);
        assert_eq!(
            version.version,
            "b3da97a449609fb4f3b14c47271b92858f2e4fa7986bfaa321a2a65ed775ae57"
        );
    }
    {
        let sver_repo = SverRepository::new(&calc_target_path(&repo, "service2")).unwrap();

        // exercise
        let sources = sver_repo.list_sources().unwrap();
        let version = sver_repo.calc_version().unwrap();

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

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "")).unwrap();

    // exercise
    let sources = sver_repo.list_sources().unwrap();
    let version = sver_repo.calc_version().unwrap();

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

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "")).unwrap();

    // exercise
    let sources = sver_repo.list_sources().unwrap();
    let version = sver_repo.calc_version().unwrap();

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

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "linkdir")).unwrap();

    // exercise
    let sources = sver_repo.list_sources().unwrap();
    let version = sver_repo.calc_version().unwrap();

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

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "linkdir")).unwrap();

    // exercise
    let sources = sver_repo.list_sources().unwrap();
    let version = sver_repo.calc_version().unwrap();

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

// repo layout
// .
// + test1.txt
// + test2.txt
// + sver.toml → [default] no setting, [prof1] exclude test1.txt
#[test]
fn multiprofile() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blog(&repo, "test1.txt", "hello".as_bytes());
    add_blog(&repo, "test2.txt", "world".as_bytes());
    add_blog(
        &repo,
        "sver.toml",
        "
        [default]
        
        [prof1]
        excludes = [
            \"test1.txt\",
        ]"
        .as_bytes(),
    );
    commit(&repo, "setup");

    // default
    {
        let sver_repo = SverRepository::new(&calc_target_path(&repo, "")).unwrap();

        // exercise
        let sources = sver_repo.list_sources().unwrap();
        let version = sver_repo.calc_version().unwrap();

        // verify
        assert_eq!(sources, vec!["sver.toml", "test1.txt", "test2.txt"]);
        assert_eq!(
            version.version,
            "f772ad1c8b70ee288c36242ce482e885d9cb0dc49f32a5c92bcad607ebe2eb23"
        );
    }

    // prof1
    {
        let sver_repo =
            SverRepository::new(&calc_target_path_with_profile(&repo, ".", "prof1")).unwrap();

        // exercise
        let sources = sver_repo.list_sources().unwrap();
        let version = sver_repo.calc_version().unwrap();

        // verify
        assert_eq!(sources, vec!["sver.toml", "test2.txt"]);
        assert_eq!(
            version.version,
            "bcc2d5c8ba9152fb12532033792c6a20d4d07a551e40477c424467c97366003a"
        );
    }
}

// repo layout
// .
// + service1/hello.txt
// + service2/sver.toml → dependency = [ "service1/hello.txt" ]
#[test]
fn valid_dependencies_repository() {
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
            \"service1/hello.txt\",
        ]"
        .as_bytes(),
    );
    commit(&repo, "setup");

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "service2")).unwrap();

    // exercise
    let mut result = sver_repo.validate_sver_config().unwrap();

    // verify
    assert_eq!(result.len(), 1);
    if let Some(ValidationResult::Valid { path, profile }) = result.pop() {
        assert_eq!(path, "service2");
        assert_eq!(profile, "default");
    } else {
        assert!(false, "this line will not be execute");
    }
}

// repo layout
// .
// + service1/hello.txt
// + service2/sver.toml → dependency = [ "service1/hello-hello.txt" ]
#[test]
fn invalid_dependencies_repository() {
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
            \"service1/hello-hello.txt\",
        ]"
        .as_bytes(),
    );
    commit(&repo, "setup");

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "service2")).unwrap();

    // exercise
    let mut result = sver_repo.validate_sver_config().unwrap();

    // verify
    assert_eq!(result.len(), 1);
    if let Some(ValidationResult::Invalid {
        path,
        profile,
        invalid_dependencies,
        invalid_excludes,
    }) = result.pop()
    {
        assert_eq!(path, "service2");
        assert_eq!(profile, "default");
        assert_eq!(invalid_dependencies, vec!["service1/hello-hello.txt"]);
        assert!(invalid_excludes.is_empty());
    } else {
        assert!(false, "this line will not be execute");
    }
}

// repo layout
// .
// + service1/hello.txt
// + service1/sver.toml → excludes = [ "hello.txt" ]
#[test]
fn valid_excludes_repository() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blog(&repo, "service1/hello.txt", "hello world!".as_bytes());
    add_blog(
        &repo,
        "service1/sver.toml",
        "
        [default]
        excludes = [
            \"hello.txt\",
        ]"
        .as_bytes(),
    );
    commit(&repo, "setup");

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "service1")).unwrap();

    // exercise
    let mut result = sver_repo.validate_sver_config().unwrap();

    // verify
    assert_eq!(result.len(), 1);
    if let Some(ValidationResult::Valid { path, profile }) = result.pop() {
        assert_eq!(path, "service1");
        assert_eq!(profile, "default");
    } else {
        assert!(false, "this line will not be execute");
    }
}

// repo layout
// .
// + service1/hello.txt
// + service1/sver.toml → excludes = [ "hello-hello.txt" ]
#[test]
fn invalid_excludes_repository() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blog(&repo, "service1/hello.txt", "hello world!".as_bytes());
    add_blog(
        &repo,
        "service1/sver.toml",
        "
        [default]
        excludes = [
            \"hello-hello.txt\",
        ]"
        .as_bytes(),
    );
    commit(&repo, "setup");

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "service1")).unwrap();

    // exercise
    let mut result = sver_repo.validate_sver_config().unwrap();

    // verify
    assert_eq!(result.len(), 1);
    if let Some(ValidationResult::Invalid {
        path,
        profile,
        invalid_dependencies,
        invalid_excludes,
    }) = result.pop()
    {
        assert_eq!(path, "service1");
        assert_eq!(profile, "default");
        assert!(invalid_dependencies.is_empty());
        assert_eq!(invalid_excludes, vec!["hello-hello.txt"]);
    } else {
        assert!(false, "this line will not be execute");
    }
}
