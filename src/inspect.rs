use anyhow::{anyhow, Context};
use inotify::WatchDescriptor;
use log::debug;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
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

pub fn inspect(
    command: String,
    args: Vec<String>,
    output: Stdio,
) -> Result<Vec<String>, anyhow::Error> {
    let repo = SverRepository::new(".").context("repository not found")?;

    let subdirs = list_subdirectories_rel(repo.work_dir());
    debug!("subdirs:{:?}", subdirs);
    let mut contain_dirs = repo.contain_directories(subdirs)?;
    contain_dirs.push(repo.work_dir().to_string());
    debug!("contain_dirs:{:?}", contain_dirs);

    let mut inotify = inotify::Inotify::init()?;
    let mut wd_path_map = BTreeMap::new();

    let mut watches = inotify.watches();
    contain_dirs.iter().for_each(|d| {
        let wd = watches.add(d, inotify::WatchMask::ACCESS).unwrap();
        wd_path_map.insert(wd, d.clone());
    });

    let (sender, receiver) = std::sync::mpsc::channel::<NotifyMessage>();
    let main_thread_sender = sender.clone();

    let sender_thread_terminator = Arc::new(AtomicBool::new(false));

    let accessed_files: Arc<Mutex<BTreeSet<String>>> = Arc::new(Mutex::new(BTreeSet::new()));
    let main_thread_accessed_files = accessed_files.clone();

    let sender_handler = {
        let sender_thread_terminator = sender_thread_terminator.clone();
        std::thread::spawn(move || {
            let mut terminate = false;
            loop {
                let mut buffer = [0; 1024];
                debug!("inotify.read_events");

                match inotify.read_events(&mut buffer) {
                    Ok(events) => {
                        debug!("inotify.read_events");
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
                    Err(e) => {
                        debug!("inotify.read_events error:{:?}", e);
                        match e.kind() {
                            std::io::ErrorKind::WouldBlock => {}
                            _ => break,
                        }
                    }
                }
                if terminate {
                    inotify.close().unwrap();
                    break;
                }
                debug!("sender_thread_terminator:{:?}", sender_thread_terminator);
                if sender_thread_terminator.load(std::sync::atomic::Ordering::Relaxed) {
                    terminate = true;
                    //break;
                }
            }
        })
    };

    let receiver_handler = std::thread::spawn(move || loop {
        match receiver.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(event) => match event {
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
            },
            Err(_err) => continue,
        }
    });

    std::process::Command::new(command)
        .args(args)
        .stdout(output)
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| anyhow!("Failed to spawn command: {}", e))?;

    // send close event and wait for receiver thread to finish
    sender_thread_terminator.store(true, std::sync::atomic::Ordering::Relaxed);
    sender_handler
        .join()
        .map_err(|e| anyhow!("sender_handler join error :{:?}", e))?;
    main_thread_sender.send(NotifyMessage::CloseEvent)?;
    receiver_handler
        .join()
        .map_err(|e| anyhow!("receiver_handler join error :{:?}", e))?;

    let accessed_files = main_thread_accessed_files.lock().unwrap();

    let mut result: Vec<String> = accessed_files
        .iter()
        .map(|f| f.trim_start_matches(repo.work_dir()).to_owned())
        .collect();
    result.sort();
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
