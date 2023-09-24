use anyhow::anyhow;
use git2::build::RepoBuilder;
use log::debug;
use std::{env::temp_dir, path::PathBuf};

use crate::sver_repository::SverRepository;

pub fn create_export_dir(export_dir: Option<String>) -> anyhow::Result<PathBuf> {
    let export_dir = if let Some(export_dir) = export_dir {
        PathBuf::from(export_dir)
    } else {
        let mut tmp_dir = temp_dir();
        tmp_dir.push(format!("sver-export-{}", uuid::Uuid::now_v7()));
        tmp_dir
    };
    if export_dir.exists() {
        return Err(anyhow!(
            "Export directory already exists. dir[{}]",
            export_dir.display()
        ));
    }
    Ok(export_dir)
}

pub fn export(path: &str, export_dir: PathBuf) -> Result<(), anyhow::Error> {
    let repo = SverRepository::new(path)?;
    let sources = repo.list_sources()?;

    {
        // If you don't drop exported_repo after cloning, the process will hold
        // the file and you won't be able to delete it in some cases on Windows,
        // so I'm making the scope clear.
        let exported_repo = RepoBuilder::new()
            .clone(repo.work_dir(), &export_dir)
            .map_err(|e| anyhow!("Failed to clone repository. err[{}]", e))?;
        let mut submodules = exported_repo.submodules()?;
        for submodule in submodules.iter_mut() {
            if let Some(submodule_path) = submodule.name() {
                debug!("submodule name: {:?}", submodule.name());
                // If it is included in sources, clone the submodule
                if sources.contains(&submodule_path.to_string()) {
                    debug!("submodule update: {:?}", submodule_path);
                    submodule.update(true, None)?;
                }
            }
        }
    }

    // Remove all files and directories except for those in sources from exported_dir and below
    let walker = walkdir::WalkDir::new(&export_dir);
    walker
        .sort_by(|a, b| a.path().cmp(b.path()))
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path() != export_dir)
        .filter(|e| {
            !sources.iter().map(|s| export_dir.join(s)).any(|s| {
                if s.is_dir() && e.path().starts_with(&s) {
                    // If the dependent file is a directory (≒ in the case of git submodule), all directories under it are left.
                    return true;
                }
                s.starts_with(e.path())
            })
        })
        .for_each(|e| {
            if !e.path().exists() {
                // noop
            } else if e.path().is_dir() {
                debug!("remove dir[{}]", e.path().display());
                std::fs::remove_dir_all(e.path()).unwrap()
            } else {
                debug!("remove file[{}]", e.path().display());
                std::fs::remove_file(e.path()).unwrap();
            }
        });

    Ok(())
}
