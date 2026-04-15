use std::fs;
use std::path::Path;
use log::{info, debug};
use wgsplit_common::error::{Result, WgsplitError};

const CGROUP_NAME: &str = "wgsplit-vpn";
const CGROUP_ROOT: &str = "/sys/fs/cgroup";

pub struct CgroupManager {
    cgroup_path: String,
    enabled: bool,
}

impl CgroupManager {
    pub fn new() -> Result<Self> {
        let cgroup_path = format!("{CGROUP_ROOT}/{CGROUP_NAME}");
        Ok(Self {
            cgroup_path,
            enabled: false,
        })
    }

    pub fn enable(&mut self) -> Result<()> {
        let path = Path::new(&self.cgroup_path);
        if !path.exists() {
            fs::create_dir(path)?;
            info!("Created cgroup {CGROUP_NAME}");
        }

        let procs_path = path.join("cgroup.procs");
        if !procs_path.exists() {
            return Err(WgsplitError::Cgroup(
                "cgroup.procs not found — is cgroups v2 mounted?".into(),
            ));
        }

        self.enabled = true;
        info!("cgroup {CGROUP_NAME} ready");
        Ok(())
    }

    pub fn disable(&mut self) -> Result<()> {
        if self.enabled {
            let path = Path::new(&self.cgroup_path);
            if path.exists() {
                let _ = fs::remove_dir(path);
            }
            self.enabled = false;
            info!("cgroup {CGROUP_NAME} removed");
        }
        Ok(())
    }

    pub fn add_pid(&self, pid: u32) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let procs_path = format!("{}/cgroup.procs", self.cgroup_path);
        debug!("Adding PID {pid} to cgroup {CGROUP_NAME}");
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .open(&procs_path)
            .map_err(|e| WgsplitError::Cgroup(format!("Failed to open cgroup.procs: {e}")))?;
        write!(f, "{pid}").map_err(|e| {
            WgsplitError::Cgroup(format!("Failed to add PID {pid} to cgroup: {e}"))
        })
    }

    pub fn remove_pid(&self, pid: u32) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        debug!("Removing PID {pid} from cgroup {CGROUP_NAME}");
        Ok(())
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}
