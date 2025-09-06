//! Resource Metrics Collection
//!
//! Provides memory and CPU monitoring capabilities for benchmarks.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::time::{Duration, Instant};

/// Combined resource metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceMetrics {
    pub memory: Option<MemoryMetrics>,
    pub cpu: Option<CpuMetrics>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Memory usage metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetrics {
    /// Peak RSS (Resident Set Size) in bytes
    pub peak_rss_bytes: u64,
    /// Current RSS in bytes
    pub current_rss_bytes: u64,
    /// Virtual memory size in bytes
    pub virtual_memory_bytes: u64,
    /// Memory samples collected during monitoring
    pub samples: Vec<MemorySample>,
}

/// CPU usage metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuMetrics {
    /// Average CPU usage percentage
    pub avg_cpu_percent: f64,
    /// Peak CPU usage percentage
    pub peak_cpu_percent: f64,
    /// CPU time in user mode (microseconds)
    pub user_time_us: u64,
    /// CPU time in kernel mode (microseconds)
    pub system_time_us: u64,
    /// CPU samples collected during monitoring
    pub samples: Vec<CpuSample>,
}

/// Serialization helper for Instant - stores as milliseconds since start
mod instant_serde {
    use super::*;
    use std::sync::OnceLock;

    static START_TIME: OnceLock<Instant> = OnceLock::new();

    pub fn serialize<S>(instant: &Instant, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let start = START_TIME.get_or_init(Instant::now);
        let millis = instant.duration_since(*start).as_millis() as u64;
        serializer.serialize_u64(millis)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Instant, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        let start = START_TIME.get_or_init(Instant::now);
        Ok(*start + Duration::from_millis(millis))
    }
}

/// Single memory measurement sample
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySample {
    #[serde(with = "instant_serde")]
    pub timestamp: Instant,
    pub rss_bytes: u64,
    pub virtual_bytes: u64,
}

/// Single CPU measurement sample
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuSample {
    #[serde(with = "instant_serde")]
    pub timestamp: Instant,
    pub cpu_percent: f64,
    pub user_time_us: u64,
    pub system_time_us: u64,
}

/// Memory usage tracker
pub struct MemoryTracker {
    samples: Vec<MemorySample>,
    monitoring: bool,
    start_time: Option<Instant>,
}

impl MemoryTracker {
    /// Create a new memory tracker
    pub fn new() -> Result<Self, MetricsError> {
        Ok(Self {
            samples: Vec::new(),
            monitoring: false,
            start_time: None,
        })
    }

    /// Start monitoring memory usage
    pub fn start_monitoring(&mut self) -> Result<(), MetricsError> {
        if self.monitoring {
            return Err(MetricsError::AlreadyMonitoring);
        }

        self.monitoring = true;
        self.start_time = Some(Instant::now());
        self.samples.clear();

        // Take initial sample
        self.take_sample()?;

        Ok(())
    }

    /// Stop monitoring and finalize metrics
    pub fn stop_monitoring(&mut self) -> Result<(), MetricsError> {
        if !self.monitoring {
            return Err(MetricsError::NotMonitoring);
        }

        // Take final sample
        self.take_sample()?;

        self.monitoring = false;
        Ok(())
    }

    /// Get collected memory metrics
    pub fn get_metrics(&self) -> Result<MemoryMetrics, MetricsError> {
        if self.samples.is_empty() {
            return Err(MetricsError::NoSamples);
        }

        let peak_rss = self.samples.iter().map(|s| s.rss_bytes).max().unwrap_or(0);
        let current_rss = self.samples.last().unwrap().rss_bytes;
        let virtual_memory = self.samples.last().unwrap().virtual_bytes;

        Ok(MemoryMetrics {
            peak_rss_bytes: peak_rss,
            current_rss_bytes: current_rss,
            virtual_memory_bytes: virtual_memory,
            samples: self.samples.clone(),
        })
    }

    /// Take a memory usage sample
    fn take_sample(&mut self) -> Result<(), MetricsError> {
        let (rss, virtual_mem) = get_memory_usage()?;

        self.samples.push(MemorySample {
            timestamp: Instant::now(),
            rss_bytes: rss,
            virtual_bytes: virtual_mem,
        });

        Ok(())
    }
}

/// CPU usage tracker
pub struct CpuTracker {
    samples: Vec<CpuSample>,
    monitoring: bool,
    start_time: Option<Instant>,
    last_user_time: u64,
    last_system_time: u64,
    last_sample_time: Option<Instant>,
}

impl CpuTracker {
    /// Create a new CPU tracker
    pub fn new() -> Result<Self, MetricsError> {
        Ok(Self {
            samples: Vec::new(),
            monitoring: false,
            start_time: None,
            last_user_time: 0,
            last_system_time: 0,
            last_sample_time: None,
        })
    }

    /// Start monitoring CPU usage
    pub fn start_monitoring(&mut self) -> Result<(), MetricsError> {
        if self.monitoring {
            return Err(MetricsError::AlreadyMonitoring);
        }

        self.monitoring = true;
        self.start_time = Some(Instant::now());
        self.samples.clear();

        // Initialize baseline measurements
        let (user, system) = get_cpu_times()?;
        self.last_user_time = user;
        self.last_system_time = system;
        self.last_sample_time = Some(Instant::now());

        Ok(())
    }

    /// Stop monitoring and finalize metrics
    pub fn stop_monitoring(&mut self) -> Result<(), MetricsError> {
        if !self.monitoring {
            return Err(MetricsError::NotMonitoring);
        }

        // Take final sample
        self.take_sample()?;

        self.monitoring = false;
        Ok(())
    }

    /// Get collected CPU metrics
    pub fn get_metrics(&self) -> Result<CpuMetrics, MetricsError> {
        if self.samples.is_empty() {
            return Err(MetricsError::NoSamples);
        }

        let avg_cpu =
            self.samples.iter().map(|s| s.cpu_percent).sum::<f64>() / self.samples.len() as f64;
        let peak_cpu = self
            .samples
            .iter()
            .map(|s| s.cpu_percent)
            .fold(0.0, f64::max);

        let total_user = self.samples.last().unwrap().user_time_us;
        let total_system = self.samples.last().unwrap().system_time_us;

        Ok(CpuMetrics {
            avg_cpu_percent: avg_cpu,
            peak_cpu_percent: peak_cpu,
            user_time_us: total_user,
            system_time_us: total_system,
            samples: self.samples.clone(),
        })
    }

    /// Take a CPU usage sample
    fn take_sample(&mut self) -> Result<(), MetricsError> {
        let now = Instant::now();
        let (user, system) = get_cpu_times()?;

        let cpu_percent = if let Some(last_time) = self.last_sample_time {
            let time_delta = now.duration_since(last_time).as_millis() as f64;
            if time_delta > 0.0 {
                let user_delta = user.saturating_sub(self.last_user_time) as f64;
                let system_delta = system.saturating_sub(self.last_system_time) as f64;
                let total_cpu_time = (user_delta + system_delta) / 1000.0; // Convert to milliseconds
                (total_cpu_time / time_delta) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        };

        self.samples.push(CpuSample {
            timestamp: now,
            cpu_percent: cpu_percent.min(100.0), // Cap at 100%
            user_time_us: user,
            system_time_us: system,
        });

        self.last_user_time = user;
        self.last_system_time = system;
        self.last_sample_time = Some(now);

        Ok(())
    }
}

/// Get current memory usage (RSS and virtual memory in bytes)
fn get_memory_usage() -> Result<(u64, u64), MetricsError> {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        let status = fs::read_to_string("/proc/self/status").map_err(|e| {
            MetricsError::SystemCall(format!("Failed to read /proc/self/status: {}", e))
        })?;

        let mut rss_kb = 0u64;
        let mut vm_size_kb = 0u64;

        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                rss_kb = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            } else if line.starts_with("VmSize:") {
                vm_size_kb = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            }
        }

        Ok((rss_kb * 1024, vm_size_kb * 1024))
    }

    #[cfg(target_os = "macos")]
    {
        use std::mem;

        // Use mach API to get memory info
        unsafe extern "C" {
            fn mach_task_self() -> u32;
            fn task_info(
                task: u32,
                flavor: i32,
                task_info: *mut u8,
                task_info_count: *mut u32,
            ) -> i32;
        }

        const TASK_BASIC_INFO: i32 = 5;
        const TASK_BASIC_INFO_COUNT: u32 = 5;

        #[repr(C)]
        struct TaskBasicInfo {
            suspend_count: u32,
            virtual_size: u64,
            resident_size: u64,
            user_time: u64,
            system_time: u64,
        }

        let mut info: TaskBasicInfo = unsafe { mem::zeroed() };
        let mut count = TASK_BASIC_INFO_COUNT;

        let result = unsafe {
            task_info(
                mach_task_self(),
                TASK_BASIC_INFO,
                &mut info as *mut _ as *mut u8,
                &mut count,
            )
        };

        if result == 0 {
            Ok((info.resident_size, info.virtual_size))
        } else {
            // Fall back to rusage if mach task_info fails
            // This can happen in some virtualized environments or with restricted permissions
            use libc::{getrusage, rusage, RUSAGE_SELF};
            let mut usage: rusage = unsafe { mem::zeroed() };
            let rusage_result = unsafe { getrusage(RUSAGE_SELF, &mut usage) };
            
            if rusage_result == 0 {
                // Convert from KB to bytes (ru_maxrss is in KB on macOS)
                let rss_bytes = usage.ru_maxrss as u64 * 1024;
                Ok((rss_bytes, rss_bytes)) // Use RSS for both resident and virtual as fallback
            } else {
                Err(MetricsError::SystemCall(format!(
                    "Both task_info (code: {}) and getrusage (code: {}) failed",
                    result, rusage_result
                )))
            }
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        // Fallback for unsupported platforms
        Ok((0, 0))
    }
}

/// Get current CPU times (user and system time in microseconds)
fn get_cpu_times() -> Result<(u64, u64), MetricsError> {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        let stat = fs::read_to_string("/proc/self/stat").map_err(|e| {
            MetricsError::SystemCall(format!("Failed to read /proc/self/stat: {}", e))
        })?;

        let fields: Vec<&str> = stat.split_whitespace().collect();
        if fields.len() >= 15 {
            let utime = fields[13].parse::<u64>().unwrap_or(0);
            let stime = fields[14].parse::<u64>().unwrap_or(0);

            // Convert from clock ticks to microseconds (assuming 100 Hz)
            let ticks_per_second = 100u64;
            let utime_us = (utime * 1_000_000) / ticks_per_second;
            let stime_us = (stime * 1_000_000) / ticks_per_second;

            Ok((utime_us, stime_us))
        } else {
            Err(MetricsError::SystemCall(
                "Invalid /proc/self/stat format".to_string(),
            ))
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::mem;

        unsafe extern "C" {
            fn getrusage(who: i32, usage: *mut rusage) -> i32;
        }

        const RUSAGE_SELF: i32 = 0;

        #[repr(C)]
        struct timeval {
            tv_sec: i64,
            tv_usec: i64,
        }

        #[repr(C)]
        struct rusage {
            ru_utime: timeval,
            ru_stime: timeval,
            // ... other fields we don't need
            _padding: [u8; 128],
        }

        let mut usage: rusage = unsafe { mem::zeroed() };
        let result = unsafe { getrusage(RUSAGE_SELF, &mut usage) };

        if result == 0 {
            let user_us = usage.ru_utime.tv_sec as u64 * 1_000_000 + usage.ru_utime.tv_usec as u64;
            let system_us =
                usage.ru_stime.tv_sec as u64 * 1_000_000 + usage.ru_stime.tv_usec as u64;
            Ok((user_us, system_us))
        } else {
            Err(MetricsError::SystemCall(format!(
                "getrusage failed with code: {}",
                result
            )))
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        // Fallback for unsupported platforms
        Ok((0, 0))
    }
}

/// Metrics collection errors
#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    #[error("Already monitoring")]
    AlreadyMonitoring,
    #[error("Not currently monitoring")]
    NotMonitoring,
    #[error("No samples collected")]
    NoSamples,
    #[error("System call failed: {0}")]
    SystemCall(String),
}
