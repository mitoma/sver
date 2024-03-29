use anyhow::{anyhow, Context};
use inotify::{Inotify, WatchDescriptor};
use log::debug;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{sleep, JoinHandle};
use std::time::Duration;

use crate::sver_repository::SverRepository;

pub fn inspect(
    path: &str,
    command: String,
    args: Vec<String>,
    output: Stdio,
) -> Result<Vec<String>, anyhow::Error> {
    let repo = SverRepository::new(path).context("repository not found")?;

    let subdirs = list_subdirectories_rel(repo.work_dir());
    debug!("subdirs:{:?}", subdirs);
    let mut git_repo_dirs = repo.contain_directories(subdirs)?;
    git_repo_dirs.push(repo.work_dir().to_string());
    debug!("contain_dirs:{:?}", git_repo_dirs);

    let thread = InotifyThread::new(&git_repo_dirs)?;

    std::process::Command::new(command)
        .args(args)
        .current_dir(path)
        .stdout(output)
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| anyhow!("Failed to spawn command: {}", e))?;

    let result = thread.terminate(repo.work_dir());
    Ok(result)
}

fn list_subdirectories_rel<P: AsRef<Path>>(path: P) -> Vec<String> {
    let str = path.as_ref().to_str().unwrap();
    let subdirectories = list_subdirectories(str);
    subdirectories
        .iter()
        .map(|s| s.strip_prefix(str).unwrap().to_string())
        .collect()
}

fn list_subdirectories<P: AsRef<Path>>(path: P) -> Vec<String> {
    use std::fs::read_dir;

    let mut subdirectories = Vec::new();
    if let Ok(entries) = read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_dir() {
                    subdirectories.push(entry.path().display().to_string());
                    subdirectories.extend(list_subdirectories(entry.path()));
                }
            }
        }
    }
    subdirectories
}

struct InotifyThread {
    thread: JoinHandle<BTreeSet<String>>,
    thread_terminator: Arc<AtomicBool>,
}

impl InotifyThread {
    fn new(dirs: &[String]) -> anyhow::Result<Self> {
        let thread_ready = Arc::new(AtomicBool::new(false));
        let thread_terminator = Arc::new(AtomicBool::new(false));

        let thread = {
            let dirs = dirs.to_owned();
            let thread_ready = thread_ready.clone();
            let thread_terminator = thread_terminator.clone();
            std::thread::spawn(move || {
                let mut inotify = inotify::Inotify::init().unwrap();
                let mut wd_path_map = BTreeMap::new();
                let mut accessed_files = BTreeSet::new();

                let mut watches = inotify.watches();
                dirs.iter().for_each(|d| {
                    let wd = watches.add(d, inotify::WatchMask::ACCESS).unwrap();
                    wd_path_map.insert(wd, d.clone());
                });
                thread_ready.store(true, Ordering::Relaxed);

                loop {
                    sleep(Duration::from_millis(1));
                    Self::read_events(&mut inotify, &mut accessed_files, &wd_path_map);
                    if thread_terminator.load(Ordering::Relaxed) {
                        inotify.close().unwrap();
                        break;
                    }
                }
                accessed_files
            })
        };
        while !thread_ready.load(Ordering::Relaxed) {
            sleep(Duration::from_millis(1));
        }
        Ok(Self {
            thread,
            thread_terminator,
        })
    }

    fn terminate(self, work_dir: &str) -> Vec<String> {
        self.thread_terminator.store(true, Ordering::Relaxed);
        let result = self.thread.join().unwrap();
        let mut result = result
            .iter()
            .map(|f| f.trim_start_matches(work_dir).to_owned())
            .collect::<Vec<String>>();
        result.sort();
        result
    }

    fn read_events(
        inotify: &mut Inotify,
        accessed_files: &mut BTreeSet<String>,
        wd_path_map: &BTreeMap<WatchDescriptor, String>,
    ) {
        let mut buffer = [0; 2048];
        if let Ok(events) = inotify.read_events(&mut buffer) {
            for event in events {
                if let Some(name) = event.name {
                    if event.mask.contains(inotify::EventMask::ACCESS)
                        && !event.mask.contains(inotify::EventMask::ISDIR)
                    {
                        let wd = event.wd;
                        let path = wd_path_map.get(&wd).unwrap();
                        let path = Path::new(path).join(name.to_string_lossy().to_string());
                        accessed_files.insert(path.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
}
