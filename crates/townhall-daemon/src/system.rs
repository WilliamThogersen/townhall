use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStats {
    pub cpu_percent: f32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub disk_total_bytes: u64,
}

#[derive(Debug)]
pub struct SystemSampler {
    path: PathBuf,
    system: Mutex<Option<System>>,
}

fn kind() -> RefreshKind {
    RefreshKind::nothing()
        .with_cpu(CpuRefreshKind::nothing().with_usage())
        .with_memory(MemoryRefreshKind::nothing().with_ram())
}

impl SystemSampler {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let system = System::new_with_specifics(kind()).ok();
        Self {
            path: path.into(),
            system: Mutex::new(system),
        }
    }

    pub fn read(&self) -> Result<SystemStats, String> {
        let (cpu_percent, memory_used_bytes, memory_total_bytes) = {
            let mut guard = self.system.lock().unwrap_or_else(|e| e.into_inner());
            if guard.is_none() {
                let system = System::new_with_specifics(kind())
                    .map_err(|e| format!("cannot read system stats: {e}"))?;
                *guard = Some(system);
            }
            let system = guard.as_mut().expect("set above");
            system.refresh_cpu_usage();
            system.refresh_memory();
            (
                system.global_cpu_usage().clamp(0.0, 100.0),
                system.used_memory(),
                system.total_memory(),
            )
        };
        let (disk_used_bytes, disk_total_bytes) = disk_usage(&self.path)?;
        Ok(SystemStats {
            cpu_percent,
            memory_used_bytes,
            memory_total_bytes,
            disk_used_bytes,
            disk_total_bytes,
        })
    }
}

fn disk_usage(path: &Path) -> Result<(u64, u64), String> {
    let real = path
        .ancestors()
        .find_map(|p| p.canonicalize().ok())
        .ok_or_else(|| format!("cannot resolve {}", path.display()))?;
    let disks = Disks::new_with_refreshed_list().map_err(|e| format!("cannot list disks: {e}"))?;
    disks
        .iter()
        .filter(|d| real.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| (d.total_space() - d.available_space(), d.total_space()))
        .ok_or_else(|| format!("no disk holds {}", real.display()))
}
