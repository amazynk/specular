/// What the machine offers. GPU details are filled in by the renderer.
#[derive(Clone, Debug, Default)]
pub struct HardwareProfile {
    pub cpu_model: String,
    pub logical_cores: usize,
    pub physical_cores: usize,
    /// Logical CPUs of the fastest core class on a hybrid CPU (P-cores, Zen 5 next to
    /// Zen 5c), empty when all cores are alike.
    pub performance_cpus: Vec<usize>,
    pub ram_total: u64,
    pub ram_available: u64,
    pub os: String,
    pub gpu_name: String,
    pub gpu_backend: String,
    pub gpu_driver: String,
    pub gpu_kind: String,
    pub max_texture_size: u32,
    pub monitor: String,
}

impl HardwareProfile {
    /// Detect CPU and memory. Cheap (reads a few small /proc files on Linux).
    pub fn detect() -> HardwareProfile {
        let logical = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let mut p = HardwareProfile {
            logical_cores: logical,
            physical_cores: logical,
            os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            ..Default::default()
        };
        #[cfg(target_os = "linux")]
        {
            if let Ok(s) = std::fs::read_to_string("/proc/cpuinfo") {
                let mut cores = std::collections::HashSet::new();
                let mut phys = String::new();
                for line in s.lines() {
                    if let Some((k, v)) = line.split_once(':') {
                        let (k, v) = (k.trim(), v.trim());
                        if k == "model name" && p.cpu_model.is_empty() {
                            p.cpu_model = v.to_string();
                        } else if k == "physical id" {
                            phys = v.to_string();
                        } else if k == "core id" {
                            cores.insert((phys.clone(), v.to_string()));
                        }
                    }
                }
                if !cores.is_empty() {
                    p.physical_cores = cores.len();
                }
            }
            p.performance_cpus = performance_cpus();
            if let Ok(s) = std::fs::read_to_string("/proc/meminfo") {
                for line in s.lines() {
                    let kb = |l: &str| {
                        l.split_whitespace()
                            .nth(1)
                            .and_then(|v| v.parse::<u64>().ok())
                            .unwrap_or(0)
                            * 1024
                    };
                    if line.starts_with("MemTotal:") {
                        p.ram_total = kb(line);
                    } else if line.starts_with("MemAvailable:") {
                        p.ram_available = kb(line);
                    }
                }
            }
            if let Ok(s) = std::fs::read_to_string("/etc/os-release") {
                for line in s.lines() {
                    if let Some(v) = line.strip_prefix("PRETTY_NAME=") {
                        p.os = v.trim_matches('"').to_string();
                    }
                }
            }
        }
        // macOS: brand string and physical cores from sysctl (core clusters: `--hw-report`);
        // Windows: from the processor's brand string and logical processor information.
        #[cfg(any(target_os = "macos", windows))]
        {
            p.cpu_model = specular_platform::cpu_brand().unwrap_or_default();
            p.physical_cores = specular_platform::physical_cores().unwrap_or(p.physical_cores);
        }
        #[cfg(not(target_os = "linux"))]
        if let Some((total, available)) = memory() {
            p.ram_total = total;
            p.ram_available = available;
        }
        if p.ram_total == 0 {
            p.ram_total = 8 << 30;
            p.ram_available = 4 << 30;
        }
        p
    }

    /// Number of logical CPUs in the fastest class (all of them on a uniform CPU).
    pub fn performance_threads(&self) -> usize {
        if self.performance_cpus.is_empty() {
            self.logical_cores
        } else {
            self.performance_cpus.len()
        }
    }

    /// Bytes of decoded pixels the RAM cache may hold.
    pub fn ram_cache_budget(&self) -> u64 {
        let avail = if self.ram_available > 0 {
            self.ram_available
        } else {
            self.ram_total / 2
        };
        (avail / 8).clamp(256 << 20, 2 << 30)
    }

    /// Worker threads for decoding (leave one core for the UI thread).
    pub fn decode_threads(&self) -> usize {
        self.logical_cores.saturating_sub(1).max(1)
    }
}

/// Memory the system can give to a new allocation without swapping, right now: Linux's
/// `MemAvailable` (free plus reclaimable caches), the equivalent elsewhere. `None` if unknown.
pub fn available_memory() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let s = std::fs::read_to_string("/proc/meminfo").ok()?;
        let line = s.lines().find(|l| l.starts_with("MemAvailable:"))?;
        let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
        Some(kb * 1024)
    }
    #[cfg(not(target_os = "linux"))]
    {
        memory().map(|m| m.1).filter(|&a| a > 0)
    }
}

/// Total and available memory through `sysinfo` (Windows: `GlobalMemoryStatusEx`; macOS:
/// the host's VM statistics), which keeps the platform calls out of this codebase.
#[cfg(not(target_os = "linux"))]
fn memory() -> Option<(u64, u64)> {
    let mut s = sysinfo::System::new();
    s.refresh_memory();
    (s.total_memory() > 0).then(|| (s.total_memory(), s.available_memory()))
}

/// CPUs whose maximum frequency is within 5% of the fastest one, when there are slower
/// ones too (hybrid CPUs); from cpufreq's `cpuinfo_max_freq`.
#[cfg(target_os = "linux")]
fn performance_cpus() -> Vec<usize> {
    let Ok(rd) = std::fs::read_dir("/sys/devices/system/cpu") else {
        return Vec::new();
    };
    let mut freqs: Vec<(usize, u64)> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let n: usize = e.file_name().to_str()?.strip_prefix("cpu")?.parse().ok()?;
            let f = std::fs::read_to_string(e.path().join("cpufreq/cpuinfo_max_freq")).ok()?;
            Some((n, f.trim().parse().ok()?))
        })
        .collect();
    freqs.sort_unstable();
    let max = freqs.iter().map(|f| f.1).max().unwrap_or(0);
    let fast: Vec<usize> = freqs
        .iter()
        .filter(|f| f.1 * 100 >= max * 95)
        .map(|f| f.0)
        .collect();
    if fast.len() == freqs.len() {
        Vec::new()
    } else {
        fast
    }
}
