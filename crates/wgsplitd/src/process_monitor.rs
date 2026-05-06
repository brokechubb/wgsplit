use std::collections::HashMap;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use log::{debug, info, warn};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe: String,
}

pub fn list_running_processes() -> Vec<ProcessInfo> {
    let mut seen: BTreeMap<String, ProcessInfo> = BTreeMap::new();
    let proc = Path::new("/proc");

    if let Ok(entries) = fs::read_dir(proc) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            let pid: u32 = match name_str.parse() {
                Ok(p) => p,
                Err(_) => continue,
            };

            let exe_path = format!("/proc/{pid}/exe");
            let target = match fs::read_link(&exe_path) {
                Ok(t) => t.to_string_lossy().to_string(),
                Err(_) => continue,
            };

            let cmd = fs::read_to_string(format!("/proc/{pid}/cmdline"))
                .unwrap_or_default()
               .replace('\0', " ")
                .trim()
                .to_string();

            let name = Path::new(&target)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| target.clone());

            seen.entry(target.clone()).or_insert(ProcessInfo {
                pid,
                name,
                exe: target,
            });
            let _ = cmd;
        }
    }

    seen.into_values().collect()
}

pub struct ProcessMonitor {
    app_paths: Vec<String>,
    pid_to_app: HashMap<u32, String>,
}

#[allow(dead_code)]
impl ProcessMonitor {
    pub fn new(app_paths: Vec<String>) -> Self {
        Self {
            app_paths,
            pid_to_app: HashMap::new(),
        }
    }

    pub fn update_app_list(&mut self, apps: Vec<String>) {
        self.app_paths = apps;
    }

    pub fn scan_all(&mut self) -> Vec<u32> {
        let mut matched = Vec::new();
        let proc = Path::new("/proc");

        if let Ok(entries) = fs::read_dir(proc) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                let pid: u32 = match name_str.parse() {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                if let Some(app) = self.check_process(pid) {
                    debug!("Found matching process: PID={pid} app={app}");
                    self.pid_to_app.insert(pid, app);
                    matched.push(pid);
                }
            }
        }

        info!("Process scan found {} matching processes", matched.len());
        matched
    }

    pub fn check_process(&self, pid: u32) -> Option<String> {
        let exe_path = format!("/proc/{pid}/exe");
        let target = fs::read_link(&exe_path).ok()?;
        let target_str = target.to_string_lossy().to_string();

        for app in &self.app_paths {
            if target_str == *app || target_str.ends_with(app) {
                return Some(app.clone());
            }
        }
        None
    }

    pub fn is_watched(&self, pid: u32) -> bool {
        self.pid_to_app.contains_key(&pid)
    }

    pub fn add_pid(&mut self, pid: u32, app: String) {
        self.pid_to_app.insert(pid, app);
    }

    pub fn remove_pid(&mut self, pid: u32) {
        self.pid_to_app.remove(&pid);
    }

    pub fn watched_pids(&self) -> Vec<u32> {
        self.pid_to_app.keys().copied().collect()
    }
}

pub fn start_process_monitor_loop(
    monitor: Arc<Mutex<ProcessMonitor>>,
    cgroup: Arc<Mutex<crate::cgroup::CgroupManager>>,
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("Failed to create process monitor runtime");
        rt.block_on(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));
            loop {
                interval.tick().await;
                if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                let procs = Path::new("/proc");
                if let Ok(entries) = fs::read_dir(procs) {
                    for entry in entries.flatten() {
                        let name = entry.file_name();
                        let name_str = name.to_string_lossy();
                        let pid: u32 = match name_str.parse() {
                            Ok(p) => p,
                            Err(_) => continue,
                        };
                        let is_watched = {
                            let mon = monitor.lock().unwrap();
                            mon.is_watched(pid)
                        };
                        if !is_watched {
                            let app = {
                                let mon = monitor.lock().unwrap();
                                mon.check_process(pid)
                            };
                            if let Some(app) = app {
                                info!(
                                    "Process {pid} ({app}) detected — adding to cgroup. \
                                     Note: any existing connections will not be rerouted."
                                );
                                let added = {
                                    let cg = cgroup.lock().unwrap();
                                    if !cg.is_enabled() {
                                        false
                                    } else {
                                        match cg.add_pid(pid) {
                                            Ok(()) => true,
                                            Err(e) => {
                                                warn!("Failed to add PID {pid} to cgroup: {e}");
                                                false
                                            }
                                        }
                                    }
                                };
                                if added {
                                    monitor.lock().unwrap().add_pid(pid, app);
                                }
                            }
                        }
                    }
                }
            }
        })
    })
}
