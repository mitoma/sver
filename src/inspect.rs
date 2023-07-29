use anyhow::anyhow;
use inotify::WatchDescriptor;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::sver_repository::SverRepository;

enum NotifyMessage {
    AccessEvent(NotifyAccessEvent),
    CloseEvent,
}

struct NotifyAccessEvent {
    path: String,
    wd: WatchDescriptor,
}

pub fn inspect(command: String, args: Vec<String>) -> Result<Vec<String>, anyhow::Error> {
    let repo = SverRepository::new(".")?;

    let subdirs = list_subdirectories_rel(repo.work_dir());
    let mut contain_dirs = repo.contain_directories(subdirs)?;
    contain_dirs.push(repo.work_dir().to_string());

    let mut inotify = inotify::Inotify::init()?;
    let mut wd_path_map = BTreeMap::new();

    let mut watches = inotify.watches();
    contain_dirs.iter().for_each(|d| {
        let wd = watches.add(d, inotify::WatchMask::ACCESS).unwrap();
        wd_path_map.insert(wd, d.clone());
    });

    let (sender, receiver) = std::sync::mpsc::channel::<NotifyMessage>();
    let main_thread_sender = sender.clone();

    let accessed_files: Arc<Mutex<BTreeSet<String>>> = Arc::new(Mutex::new(BTreeSet::new()));
    let main_thread_accessed_files = accessed_files.clone();

    let _sender_handler = std::thread::spawn(move || loop {
        let mut buffer = [0; 1024];
        match inotify.read_events_blocking(&mut buffer) {
            Ok(events) => {
                for event in events {
                    if let Some(name) = event.name {
                        if event.mask.contains(inotify::EventMask::ACCESS)
                            && !event.mask.contains(inotify::EventMask::ISDIR)
                        {
                            let e = NotifyAccessEvent {
                                path: name.to_string_lossy().to_string(),
                                wd: event.wd,
                            };
                            sender.send(NotifyMessage::AccessEvent(e)).unwrap();
                        }
                    }
                }
            }
            Err(_) => {
                break;
            }
        }
    });

    let receiver_handler = std::thread::spawn(move || {
        while let Ok(event) = receiver.recv() {
            match event {
                NotifyMessage::AccessEvent(e) => {
                    let path = wd_path_map.get(&e.wd).unwrap();
                    let path = Path::new(path).join(e.path);
                    accessed_files
                        .lock()
                        .unwrap()
                        .insert(path.to_string_lossy().to_string());
                }
                NotifyMessage::CloseEvent => {
                    break;
                }
            }
        }
    });

    std::process::Command::new(command)
        .args(args)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| anyhow!("Failed to spawn command: {}", e))?;

    // send close event and wait for receiver thread to finish
    main_thread_sender.send(NotifyMessage::CloseEvent).unwrap();
    receiver_handler.join().unwrap();

    let accessed_files = main_thread_accessed_files.lock().unwrap();
    Ok(accessed_files.iter().map(|f| f.to_string()).collect())
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
