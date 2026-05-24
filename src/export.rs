use anyhow::anyhow;
use git2::build::RepoBuilder;
use log::debug;
use std::{
    env::temp_dir,
    path::{Path, PathBuf},
};

use crate::sver_repository::SverRepository;

fn collect_paths_recursive(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut result = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;

            result.push(path.clone());
            if file_type.is_dir() {
                stack.push(path);
            }
        }
    }

    result.sort();
    Ok(result)
}

fn should_keep_path(path: &Path, source_paths: &[PathBuf]) -> bool {
    source_paths.iter().any(|s| {
        if s.is_dir() && path.starts_with(s) {
            // source がディレクトリなら、その配下はすべて残す。
            return true;
        }
        // source 自体、または source に到達する親ディレクトリは残す。
        s.starts_with(path)
    })
}

fn depth(path: &Path) -> usize {
    path.components().count()
}

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
            if let Ok(submodule_path) = submodule.name() {
                debug!("submodule name: {:?}", submodule_path);
                // If it is included in sources, clone the submodule
                if sources.contains(&submodule_path.to_string()) {
                    debug!("submodule update: {:?}", submodule_path);
                    submodule.update(true, None)?;
                }
            }
        }
    }

    // Remove all files and directories except for those in sources from exported_dir and below
    let source_paths: Vec<PathBuf> = sources.iter().map(|s| export_dir.join(s)).collect();
    let mut remove_targets: Vec<PathBuf> = collect_paths_recursive(&export_dir)?
        .into_iter()
        .filter(|p| p != &export_dir)
        .filter(|p| !should_keep_path(p, &source_paths))
        .collect();

    // 深い階層から削除すると、親ディレクトリ削除後の race を避けやすい。
    remove_targets.sort_by(|a, b| depth(b).cmp(&depth(a)).then_with(|| a.cmp(b)));

    for path in remove_targets {
        if !path.exists() {
            continue;
        }
        if path.is_dir() {
            debug!("remove dir[{}]", path.display());
            std::fs::remove_dir_all(&path)?;
        } else {
            debug!("remove file[{}]", path.display());
            std::fs::remove_file(&path)?;
        }
    }

    Ok(())
}
