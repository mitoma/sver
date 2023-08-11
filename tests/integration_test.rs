mod test_tool;

use std::{env::temp_dir, fs::create_dir};

use chrono::{TimeZone, Utc};
use git2::Repository;
use log::debug;
use sver::sver_repository::ValidationResults;
use sver::{
    sver_config::{CalculationTarget, ValidationResult},
    sver_repository::SverRepository,
};
use test_tool::commit_at;
use uuid::Uuid;

use crate::test_tool::{
    add_blob, add_blob_executable, add_submodule, add_symlink, calc_target_path,
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
    add_blob(&repo, "hello.txt", "hello world!".as_bytes());
    add_blob(&repo, "service1/world.txt", "good morning!".as_bytes());
    commit(&repo, "setup");

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "")).unwrap();

    // exercise
    let sources = sver_repo.list_sources().unwrap();
    let version = sver_repo.calc_version().unwrap();

    // verify
    assert_eq!(sources, vec!["hello.txt", "service1/world.txt"]);
    assert_eq!(
        version.version,
        "d601cac0967b58cd86a3a0384709f81ada1db3a42060e4458b843a7c7613b6ea"
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
    add_blob_executable(&repo, "hello.txt", "hello world!".as_bytes());
    add_blob(&repo, "service1/world.txt", "good morning!".as_bytes());
    commit(&repo, "setup");

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "")).unwrap();

    // exercise
    let sources = sver_repo.list_sources().unwrap();
    let version = sver_repo.calc_version().unwrap();

    // verify
    assert_eq!(sources, vec!["hello.txt", "service1/world.txt"]);
    assert_eq!(
        version.version,
        "12890ee3efefa6318fbbd29adc708031c3b3a5080b8d195fb5c124080c3ec6c4"
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
    add_blob(&repo, "service1/hello.txt", "hello world!".as_bytes());
    add_blob(
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
        "edcd58dca3b80c45676296640e0f64a11366cc4762247cf3b8873e17b3328648"
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
    add_blob(
        &repo,
        "service1/sver.toml",
        "
        [default]
        dependencies = [
            \"service2\",
        ]"
        .as_bytes(),
    );
    add_blob(
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
            "60163d9d178386ea7055374d104cbea3712bbdeb3c3dd5931ddf67dd7c8f5cdb"
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
            "4241b717612be4a8f64f418d0bc2e568c1d3d4a01f42d88933b14bfbd585b90e"
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
    add_blob(&repo, "hello.txt", "hello".as_bytes());
    add_blob(
        &repo,
        "sver.toml",
        "
        [default]
        excludes = [
            \"doc\",
        ]"
        .as_bytes(),
    );
    add_blob(&repo, "doc/README.txt", "README".as_bytes());
    commit(&repo, "setup");

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "")).unwrap();

    // exercise
    let sources = sver_repo.list_sources().unwrap();
    let version = sver_repo.calc_version().unwrap();

    // verify
    assert_eq!(sources, vec!["hello.txt", "sver.toml"]);
    assert_eq!(
        version.version,
        "8b883e40e964120ffb2f577e782b3a491156b07ace162d78a5434638133f13a0"
    );
}

// repo layout
// .
// + sub → submodule ../sub e40a885afd013606e105c027a5c31910137e5566
#[test]
fn has_submodule() {
    initialize();

    // setup
    let mut tmp_dir = temp_dir();
    let uuid = Uuid::new_v4();
    tmp_dir.push(format!("sver-{}", uuid));
    create_dir(tmp_dir.clone()).unwrap();

    // setup external repo
    let mut sub_repo_dir = tmp_dir.clone();
    sub_repo_dir.push("sub");

    let sub_repo = Repository::init(sub_repo_dir).unwrap();
    add_blob(&sub_repo, "hello.txt", "hello".as_bytes());
    commit_at(
        &sub_repo,
        "setup",
        Utc.with_ymd_and_hms(2022, 10, 1, 10, 20, 30)
            .earliest()
            .unwrap(),
    );

    // setup sut repo
    let mut sut_repo_dir = tmp_dir.clone();
    sut_repo_dir.push("sut");

    let mut repo = Repository::init(sut_repo_dir).unwrap();
    add_submodule(
        &mut repo,
        "../sub",
        "sub",
        "e40a885afd013606e105c027a5c31910137e5566",
    );
    commit(&repo, "setup");

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "")).unwrap();

    // exercise
    let sources = sver_repo.list_sources().unwrap();
    let version = sver_repo.calc_version().unwrap();

    // verify
    assert_eq!(sources, vec![".gitmodules", "sub"]);
    assert_eq!(
        version.version,
        "975af38bee93750b69eed48da18f3041058bacd90e215fb61f920c1e9cb710b7"
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
    add_blob(&repo, "original/README.txt", "hello.world".as_bytes());
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
        "2d092ad213e284863e66125b9fda9e642a50c8347e640d5f431e587fde83bf93"
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
    add_blob(&repo, "original/README.txt", "hello.world".as_bytes());
    add_blob(&repo, "original/Sample.txt", "sample".as_bytes());

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
        "bfd875f92865460d1fcff4769bcd39e7c894c196265ec89937ca05505b41c935"
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
    add_blob(&repo, "test1.txt", "hello".as_bytes());
    add_blob(&repo, "test2.txt", "world".as_bytes());
    add_blob(
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
            "6594bb8e093129d224a6055d8484cca4138124c3014ac5c6586cb1f73d0849f7"
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
            "9119cebdb5271d79539355318a02488e6c7b7f54dabe120a55220482f48a386f"
        );
    }
}

// repo layout
// .
// + lib1/test1.txt
// + lib1/test2.txt
// + lib1/sver.toml → [default] no setting, [prof1] excludes = ["test2.txt"]
// + lib2/sver.toml → [default] no setting, [prof2] dependency = ["lib1:prof1"], [prof3] dependency = ["lib1/test2.txt"]
#[test]
fn multiprofile_multidir() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blob(&repo, "lib1/test1.txt", "hello".as_bytes());
    add_blob(&repo, "lib1/test2.txt", "world".as_bytes());
    add_blob(
        &repo,
        "lib1/sver.toml",
        "
        [default]
        
        [prof1]
        excludes = [
            \"test2.txt\",
        ]"
        .as_bytes(),
    );
    add_blob(
        &repo,
        "lib2/sver.toml",
        "
        [default]
        
        [prof2]
        dependencies = [
            \"lib1:prof1\",
        ]

        [prof3]
        dependencies = [
            \"lib1/test2.txt\",
        ]"
        .as_bytes(),
    );
    commit(&repo, "setup");

    // default
    {
        let sver_repo = SverRepository::new(&calc_target_path(&repo, "lib1")).unwrap();

        // exercise
        let sources = sver_repo.list_sources().unwrap();
        let version = sver_repo.calc_version().unwrap();

        // verify
        assert_eq!(
            sources,
            vec!["lib1/sver.toml", "lib1/test1.txt", "lib1/test2.txt"]
        );
        assert_eq!(
            version.version,
            "353265a18ba62fe6a818e8b35967706e356e2975ebbb439ecd969a57b3c8b95a"
        );
    }
    // prof1
    {
        let sver_repo = SverRepository::new(&calc_target_path(&repo, "lib1:prof1")).unwrap();

        // exercise
        let sources = sver_repo.list_sources().unwrap();
        let version = sver_repo.calc_version().unwrap();

        // verify
        assert_eq!(sources, vec!["lib1/sver.toml", "lib1/test1.txt"]);
        assert_eq!(
            version.version,
            "ee87ef59413a2072ab99e14495a6995af3ffd5aaea193d43d08264f717758a38"
        );
    }
    // prof2
    {
        let sver_repo = SverRepository::new(&calc_target_path(&repo, "lib2:prof2")).unwrap();

        // exercise
        let sources = sver_repo.list_sources().unwrap();
        let version = sver_repo.calc_version().unwrap();

        // verify
        assert_eq!(
            sources,
            vec!["lib1/sver.toml", "lib1/test1.txt", "lib2/sver.toml"]
        );
        assert_eq!(
            version.version,
            "7403ad568d8781658870c471a52dd9c51aae3297965b6dded2f3afb25e3b282b"
        );
    }
    // prof2
    {
        let sver_repo = SverRepository::new(&calc_target_path(&repo, "lib2:prof3")).unwrap();

        // exercise
        let sources = sver_repo.list_sources().unwrap();
        let version = sver_repo.calc_version().unwrap();

        // verify
        assert_eq!(sources, vec!["lib1/test2.txt", "lib2/sver.toml"]);
        assert_eq!(
            version.version,
            "283c470015f5791d8bcdd0c924d38488b7106be7ed4138d3e339b4cc2b5ffc9e"
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
    add_blob(&repo, "service1/hello.txt", "hello world!".as_bytes());
    add_blob(
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
    let ValidationResults {
        has_invalid,
        mut results,
    } = sver_repo.validate_sver_config().unwrap();

    // verify
    assert_eq!(has_invalid, false);
    assert_eq!(results.len(), 1);
    if let Some(ValidationResult::Valid {
        calcuration_target: CalculationTarget { path, profile },
    }) = results.pop()
    {
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
    add_blob(&repo, "service1/hello.txt", "hello world!".as_bytes());
    add_blob(
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
    let ValidationResults {
        has_invalid,
        mut results,
    } = sver_repo.validate_sver_config().unwrap();

    // verify
    assert_eq!(has_invalid, true);
    assert_eq!(results.len(), 1);
    if let Some(ValidationResult::Invalid {
        calcuration_target: CalculationTarget { path, profile },
        invalid_dependencies,
        invalid_excludes,
    }) = results.pop()
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
    add_blob(&repo, "service1/hello.txt", "hello world!".as_bytes());
    add_blob(
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
    let ValidationResults {
        has_invalid,
        mut results,
    } = sver_repo.validate_sver_config().unwrap();

    // verify
    assert_eq!(has_invalid, false);
    assert_eq!(results.len(), 1);
    if let Some(ValidationResult::Valid {
        calcuration_target: CalculationTarget { path, profile },
    }) = results.pop()
    {
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
    add_blob(&repo, "service1/hello.txt", "hello world!".as_bytes());
    add_blob(
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
    let ValidationResults {
        has_invalid,
        mut results,
    } = sver_repo.validate_sver_config().unwrap();

    // verify
    assert_eq!(has_invalid, true);
    assert_eq!(results.len(), 1);
    if let Some(ValidationResult::Invalid {
        calcuration_target: CalculationTarget { path, profile },
        invalid_dependencies,
        invalid_excludes,
    }) = results.pop()
    {
        assert_eq!(path, "service1");
        assert_eq!(profile, "default");
        assert!(invalid_dependencies.is_empty());
        assert_eq!(invalid_excludes, vec!["hello-hello.txt"]);
    } else {
        assert!(false, "this line will not be execute");
    }
}

// repo layout
// .
// + service1/hello.txt
// + service2/sver.toml → [prof1] dependency = [ "service1/hello.txt" ]
#[test]
fn valid_has_profile_repository() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blob(&repo, "service1/hello.txt", "hello world!".as_bytes());
    add_blob(
        &repo,
        "service2/sver.toml",
        "
        [default]
        [prof1]
        dependencies = [
            \"service1/hello.txt\",
        ]"
        .as_bytes(),
    );
    commit(&repo, "setup");

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "service2")).unwrap();

    // exercise
    let ValidationResults {
        has_invalid,
        mut results,
    } = sver_repo.validate_sver_config().unwrap();

    // verify
    assert_eq!(has_invalid, false);
    assert_eq!(results.len(), 2);
    if let Some(ValidationResult::Valid {
        calcuration_target: CalculationTarget { path, profile },
    }) = results.pop()
    {
        assert_eq!(path, "service2");
        assert_eq!(profile, "prof1");
    } else {
        assert!(false, "this line will not be execute");
    }
    if let Some(ValidationResult::Valid {
        calcuration_target: CalculationTarget { path, profile },
    }) = results.pop()
    {
        assert_eq!(path, "service2");
        assert_eq!(profile, "default");
    } else {
        assert!(false, "this line will not be execute");
    }
}

// repo layout
// .
// + service1/hello.txt
// + service2/sver.toml → [prof1] dependency = [ "service1/hello.txt" ]
#[test]
fn invalid_has_profile_repository() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blob(&repo, "service1/hello.txt", "hello world!".as_bytes());
    add_blob(
        &repo,
        "service2/sver.toml",
        "
        [default]
        [prof1]
        dependencies = [
            \"service1/helloo.txt\",
        ]"
        .as_bytes(),
    );
    commit(&repo, "setup");

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "service2")).unwrap();

    // exercise
    let ValidationResults {
        has_invalid,
        mut results,
    } = sver_repo.validate_sver_config().unwrap();

    // verify
    assert_eq!(has_invalid, true);
    assert_eq!(results.len(), 2);
    if let Some(ValidationResult::Invalid {
        calcuration_target: CalculationTarget { path, profile },
        invalid_dependencies,
        ..
    }) = results.pop()
    {
        assert_eq!(path, "service2");
        assert_eq!(profile, "prof1");
        assert_eq!(invalid_dependencies, vec!["service1/helloo.txt"]);
    } else {
        assert!(false, "this line will not be execute");
    }
    if let Some(ValidationResult::Valid {
        calcuration_target: CalculationTarget { path, profile },
    }) = results.pop()
    {
        assert_eq!(path, "service2");
        assert_eq!(profile, "default");
    } else {
        assert!(false, "this line will not be execute");
    }
}

// repo layout
// .
// + service1/sver.toml → [prof1]
// + service2/sver.toml → [prof2] dependency = [ "service1:prof1" ]
#[test]
fn valid_no_target_profile_repository() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blob(
        &repo,
        "service1/sver.toml",
        "
        [default]
        [prof1]
        "
        .as_bytes(),
    );
    add_blob(
        &repo,
        "service2/sver.toml",
        "
        [default]
        [prof2]
        dependencies = [
            \"service1:prof1\",
        ]"
        .as_bytes(),
    );
    commit(&repo, "setup");

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "service2")).unwrap();

    // exercise
    let ValidationResults {
        has_invalid,
        mut results,
    } = sver_repo.validate_sver_config().unwrap();

    // verify
    assert_eq!(has_invalid, false);
    debug!("{:?}", results);
    assert_eq!(results.len(), 4);
    if let Some(ValidationResult::Valid {
        calcuration_target: CalculationTarget { path, profile },
    }) = results.pop()
    {
        assert_eq!(path, "service2");
        assert_eq!(profile, "prof2");
    } else {
        assert!(false, "this line will not be execute");
    }
    if let Some(ValidationResult::Valid {
        calcuration_target: CalculationTarget { path, profile },
    }) = results.pop()
    {
        assert_eq!(path, "service2");
        assert_eq!(profile, "default");
    } else {
        assert!(false, "this line will not be execute");
    }
}

// repo layout
// .
// + service1/sver.toml → [prof1]
// + service2/sver.toml → [prof2] dependency = [ "service1:prof1" ]
#[test]
fn invalid_no_target_profile_repository() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blob(
        &repo,
        "service1/sver.toml",
        "
        [default]
        [prof1]
        "
        .as_bytes(),
    );
    add_blob(
        &repo,
        "service2/sver.toml",
        "
        [default]
        [prof2]
        dependencies = [
            \"service1:prof999\",
        ]
        [prof3]
        dependencies = [
            \"service1/:prof999\",
        ]"
        .as_bytes(),
    );
    commit(&repo, "setup");

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "service2")).unwrap();

    // exercise
    let ValidationResults {
        has_invalid,
        mut results,
    } = sver_repo.validate_sver_config().unwrap();

    // verify
    assert_eq!(has_invalid, true);
    debug!("{:?}", results);
    assert_eq!(results.len(), 5);
    if let Some(ValidationResult::Invalid {
        calcuration_target: CalculationTarget { path, profile },
        invalid_dependencies,
        ..
    }) = results.pop()
    {
        assert_eq!(path, "service2");
        assert_eq!(profile, "prof3");
        assert_eq!(invalid_dependencies, vec!["service1/:prof999"]);
    } else {
        assert!(false, "this line will not be execute");
    }
    if let Some(ValidationResult::Invalid {
        calcuration_target: CalculationTarget { path, profile },
        invalid_dependencies,
        ..
    }) = results.pop()
    {
        assert_eq!(path, "service2");
        assert_eq!(profile, "prof2");
        assert_eq!(invalid_dependencies, vec!["service1:prof999"]);
    } else {
        assert!(false, "this line will not be execute");
    }
    if let Some(ValidationResult::Valid {
        calcuration_target: CalculationTarget { path, profile },
    }) = results.pop()
    {
        assert_eq!(path, "service2");
        assert_eq!(profile, "default");
    } else {
        assert!(false, "this line will not be execute");
    }
}

// repo layout
// .
// + service1/sver.toml → no default
// + service2/sver.toml → dependency = [ "service1:default" ]
#[test]
fn invalid_no_default_repository() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blob(
        &repo,
        "service1/sver.toml",
        "
        [no-default]
        dependencies = []"
            .as_bytes(),
    );
    add_blob(
        &repo,
        "service2/sver.toml",
        "
        [default]
        dependencies = [
            \"service1:default\",
        ]"
        .as_bytes(),
    );
    commit(&repo, "setup");

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "service2")).unwrap();

    // exercise
    let ValidationResults {
        has_invalid,
        mut results,
    } = sver_repo.validate_sver_config().unwrap();

    // verify
    assert_eq!(has_invalid, true);
    assert_eq!(results.len(), 2);

    if let Some(ValidationResult::Invalid {
        calcuration_target: CalculationTarget { profile, path },
        ..
    }) = results.pop()
    {
        assert_eq!(path, "service2");
        assert_eq!(profile, "default");
    } else {
        assert!(false, "this line will not be execute");
    }

    if let Some(ValidationResult::Valid {
        calcuration_target: CalculationTarget { profile, path },
        ..
    }) = results.pop()
    {
        assert_eq!(path, "service1");
        assert_eq!(profile, "no-default");
    } else {
        assert!(false, "this line will not be execute");
    }
}

// repo layout
// .
// + service1/README.md → no config file
// + service2/sver.toml → dependency = [ "service1:default" ]
#[test]
fn valid_ref_to_no_config_repository() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blob(&repo, "service1/README.md", "hello".as_bytes());
    add_blob(
        &repo,
        "service2/sver.toml",
        "
        [default]
        dependencies = [
            \"service1:default\",
        ]"
        .as_bytes(),
    );
    commit(&repo, "setup");

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "service2")).unwrap();

    // exercise
    let ValidationResults {
        has_invalid,
        mut results,
    } = sver_repo.validate_sver_config().unwrap();

    // verify
    assert_eq!(has_invalid, false);
    assert_eq!(results.len(), 1);

    if let Some(ValidationResult::Valid {
        calcuration_target: CalculationTarget { profile, path },
        ..
    }) = results.pop()
    {
        assert_eq!(path, "service2");
        assert_eq!(profile, "default");
    } else {
        assert!(false, "this line will not be execute");
    }
}

// repo layout
// .
// + service1/hello.txt
#[test]
fn init_on_basedirectory() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blob(&repo, "service1/hello.txt", "world".as_bytes());
    commit(&repo, "setup");

    let sver_repo = SverRepository::new(&calc_target_path(&repo, ".")).unwrap();

    // exercise
    let result = sver_repo.init_sver_config();

    // verify
    debug!("{:?}", result);
    assert_eq!(result.unwrap(), "sver.toml is generated. path:");
}

// repo layout
// .
// + service1/hello.txt
#[test]
fn init_on_subdirectory() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blob(&repo, "service1/hello.txt", "world".as_bytes());
    commit(&repo, "setup");

    let sver_repo = SverRepository::new(&calc_target_path(&repo, "service1")).unwrap();

    // exercise
    let result = sver_repo.init_sver_config();

    // verify
    debug!("{:?}", result);
    assert_eq!(result.unwrap(), "sver.toml is generated. path:service1");
}

// repo layout
// .
// + test1.txt
// + test2.txt
// + lib/sver.toml -> [default] dependency = ["lib/:prof1","lib/:prof2"], [prof1] dependency = ["test1.txt"], [prof2] dependency = ["test2.txt"]
#[test]
fn multiprofile_singledir() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blob(&repo, "test1.txt", "hello".as_bytes());
    add_blob(&repo, "test2.txt", "world".as_bytes());
    add_blob(
        &repo,
        "lib/sver.toml",
        "
        [default]
        dependencies = [
            \"lib/:prof1\",
            \"lib/:prof2\",
        ]

        [prof1]
        dependencies = [
            \"test1.txt\",
        ]

        [prof2]
        dependencies = [
            \"test2.txt\",
        ]"
        .as_bytes(),
    );
    commit(&repo, "setup");

    // default
    {
        let sver_repo = SverRepository::new(&calc_target_path(&repo, "lib")).unwrap();

        // exercise
        let sources = sver_repo.list_sources().unwrap();
        let version = sver_repo.calc_version().unwrap();

        // verify
        assert_eq!(sources, vec!["lib/sver.toml", "test1.txt", "test2.txt"]);
        assert_eq!(
            version.version,
            "219fa5cd7cc287ff9f3df5b96be5b8e8d81decc95ba69d13e67a722a9bf45c31"
        );
    }
}

// repo layout
// .
// + src/test1.txt
// + src/test2.txt
// + src/sver.toml ->
//      [prof1] excludes = ["test2.txt"]
//      [prof2] excludes = ["test1.txt"]
// + lib/sver.toml ->
//      [default] dependency = ["src/:prof1","src/:prof2"]
#[test]
fn multiprofile_ref_singledir() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blob(&repo, "src/test1.txt", "hello".as_bytes());
    add_blob(&repo, "src/test2.txt", "world".as_bytes());
    add_blob(
        &repo,
        "src/sver.toml",
        "
        [prof1]
        excludes = [
            \"test2.txt\",
        ]

        [prof2]
        excludes = [
            \"test1.txt\",
        ]"
        .as_bytes(),
    );
    add_blob(
        &repo,
        "lib/sver.toml",
        "
        [default]
        dependencies = [
            \"src:prof1\",
            \"src:prof2\",
        ]"
        .as_bytes(),
    );
    commit(&repo, "setup");

    // src:prof1
    {
        let sver_repo = SverRepository::new(&calc_target_path(&repo, "src:prof1")).unwrap();
        // exercise
        let sources = sver_repo.list_sources().unwrap();
        // verify
        assert_eq!(sources, vec!["src/sver.toml", "src/test1.txt"]);
    }
    // src:prof2
    {
        let sver_repo = SverRepository::new(&calc_target_path(&repo, "src:prof2")).unwrap();
        // exercise
        let sources = sver_repo.list_sources().unwrap();
        // verify
        assert_eq!(sources, vec!["src/sver.toml", "src/test2.txt"]);
    }

    // default
    {
        let sver_repo = SverRepository::new(&calc_target_path(&repo, "lib")).unwrap();

        // exercise
        let sources = sver_repo.list_sources().unwrap();
        let version = sver_repo.calc_version().unwrap();

        // verify
        assert_eq!(
            sources,
            vec![
                "lib/sver.toml",
                "src/sver.toml",
                "src/test1.txt",
                "src/test2.txt"
            ]
        );
        assert_eq!(
            version.version,
            "9f70fc2af283722f7ec609b4b7bb36b0f6c16699036f516f04ebff7c91dd2afc"
        );
    }
}

// repo layout
// .
// + test1.txt
// + src/test2.txt
// + lib/test3.txt
#[cfg(target_os = "linux")]
#[test]
fn inspect_test_1() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blob(&repo, "test1.txt", "hello".as_bytes());
    add_blob(&repo, "src/test2.txt", "world".as_bytes());
    add_blob(&repo, "lib/test3.txt", "morning".as_bytes());
    commit(&repo, "setup");
    std::env::set_current_dir(repo.workdir().unwrap()).unwrap();

    // exercise
    let result =
        sver::inspect::inspect("ls".to_string(), vec![], std::process::Stdio::null()).unwrap();

    // verify
    assert_eq!(result, Vec::<String>::new());
}

// repo layout
// .
// + test1.txt
// + src/test2.txt
// + lib/test3.txt
#[cfg(target_os = "linux")]
#[test]
fn inspect_test_2() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blob(&repo, "test1.txt", "hello".as_bytes());
    add_blob(&repo, "src/test2.txt", "world".as_bytes());
    add_blob(&repo, "lib/test3.txt", "morning".as_bytes());
    commit(&repo, "setup");
    std::env::set_current_dir(repo.workdir().unwrap()).unwrap();

    // exercise
    let result = sver::inspect::inspect(
        "cat".to_string(),
        vec!["test1.txt".to_string()],
        std::process::Stdio::null(),
    )
    .unwrap();
    // verify
    assert_eq!(result, vec!["test1.txt"]);
}

// repo layout
// .
// + test1.txt
// + src/test2.txt
// + lib/test3.txt
#[cfg(target_os = "linux")]
#[test]
fn inspect_test_3() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blob(&repo, "test1.txt", "hello".as_bytes());
    add_blob(&repo, "src/test2.txt", "world".as_bytes());
    add_blob(&repo, "lib/test3.txt", "morning".as_bytes());
    commit(&repo, "setup");
    std::env::set_current_dir(repo.workdir().unwrap()).unwrap();

    // exercise
    let result = sver::inspect::inspect(
        "cat".to_string(),
        vec!["src/test2.txt".to_string(), "lib/test3.txt".to_string()],
        std::process::Stdio::null(),
    )
    .unwrap();

    //verify
    assert_eq!(result, vec!["lib/test3.txt", "src/test2.txt"]);
}

// repo layout
// .
// + test1.txt
// + src/test2.txt
// + lib/test3.txt
#[cfg(target_os = "linux")]
#[test]
fn inspect_test_4() {
    initialize();

    // setup
    let repo = setup_test_repository();
    add_blob(&repo, "test1.txt", "hello".as_bytes());
    add_blob(&repo, "src/test2.txt", "world".as_bytes());
    add_blob(&repo, "lib/test3.txt", "morning".as_bytes());
    commit(&repo, "setup");
    std::env::set_current_dir(repo.workdir().unwrap()).unwrap();

    // exercise
    let result = sver::inspect::inspect(
        "sh".to_string(),
        vec![
            "-c".to_string(),
            "touch src/test4.txt && cat src/test4.txt".to_string(),
        ],
        std::process::Stdio::null(),
    )
    .unwrap();

    // verify
    assert_eq!(result, Vec::<String>::new());
}
