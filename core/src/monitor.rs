use std::sync::{Arc, Mutex};
use std::time::Instant;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, Pid, ProcessRefreshKind, RefreshKind, System};
use tokio::time::{interval, Duration};

#[derive(Debug, Clone)]
pub struct ResourceSample {
    pub timestamp: f64,
    pub cpu_usage: f32,
    pub memory_mb: f64,
    pub thread_count: usize,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct ResourceStats {
    pub samples: Vec<ResourceSample>,
    pub peak_cpu: f32,
    pub peak_memory_mb: f64,
    pub avg_cpu: f32,
    pub avg_memory_mb: f64,
    pub peak_threads: usize,
    pub avg_threads: f32,
    pub total_disk_read_mb: f64,
    pub total_disk_write_mb: f64,
    pub load_avg_1min: f64,
}

impl ResourceStats {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
            peak_cpu: 0.0,
            peak_memory_mb: 0.0,
            avg_cpu: 0.0,
            avg_memory_mb: 0.0,
            peak_threads: 0,
            avg_threads: 0.0,
            total_disk_read_mb: 0.0,
            total_disk_write_mb: 0.0,
            load_avg_1min: 0.0,
        }
    }

    pub fn add_sample(&mut self, sample: ResourceSample) {
        self.peak_cpu = self.peak_cpu.max(sample.cpu_usage);
        self.peak_memory_mb = self.peak_memory_mb.max(sample.memory_mb);
        self.peak_threads = self.peak_threads.max(sample.thread_count);

        if let Some(last) = self.samples.last() {
            self.total_disk_read_mb += (sample.disk_read_bytes.saturating_sub(last.disk_read_bytes))
                as f64
                / 1024.0
                / 1024.0;
            self.total_disk_write_mb += (sample
                .disk_write_bytes
                .saturating_sub(last.disk_write_bytes))
                as f64
                / 1024.0
                / 1024.0;
        }

        self.samples.push(sample);
        self.update_averages();
    }

    fn update_averages(&mut self) {
        if self.samples.is_empty() {
            return;
        }

        let count = self.samples.len() as f32;

        self.avg_cpu = self.samples.iter().map(|s| s.cpu_usage).sum::<f32>() / count;
        self.avg_memory_mb = self.samples.iter().map(|s| s.memory_mb).sum::<f64>() / count as f64;
        self.avg_threads = self
            .samples
            .iter()
            .map(|s| s.thread_count as f32)
            .sum::<f32>()
            / count;
    }
}

#[derive(Clone)]
pub struct ResourceMonitor {
    stats: Arc<Mutex<ResourceStats>>,
    tracked_pids: Arc<Mutex<Vec<Pid>>>,
    start_time: Instant,
}

impl ResourceMonitor {
    pub fn new() -> Self {
        Self {
            stats: Arc::new(Mutex::new(ResourceStats::new())),
            tracked_pids: Arc::new(Mutex::new(Vec::new())),
            start_time: Instant::now(),
        }
    }

    pub fn add_pid(&self, pid: u32) {
        if let Ok(mut pids) = self.tracked_pids.lock() {
            pids.push(Pid::from_u32(pid));
        }
    }

    pub fn remove_pid(&self, pid: u32) {
        if let Ok(mut pids) = self.tracked_pids.lock() {
            pids.retain(|p| p.as_u32() != pid);
        }
    }

    pub fn start_monitoring(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut sys = System::new_with_specifics(
                RefreshKind::new()
                    .with_cpu(CpuRefreshKind::everything())
                    .with_memory(MemoryRefreshKind::everything())
                    .with_processes(
                        ProcessRefreshKind::new()
                            .with_cpu()
                            .with_memory()
                            .with_disk_usage(),
                    ),
            );

            let mut interval = interval(Duration::from_millis(500));

            loop {
                interval.tick().await;

                sys.refresh_cpu();
                sys.refresh_memory();
                sys.refresh_processes();

                let pids = self.tracked_pids.lock().unwrap().clone();

                let mut total_cpu = 0.0f32;
                let mut total_memory = 0.0f64;
                let mut total_threads = 0usize;
                let mut total_disk_read = 0u64;
                let mut total_disk_write = 0u64;

                for pid in &pids {
                    if let Some(process) = sys.process(*pid) {
                        total_cpu += process.cpu_usage();
                        total_memory += process.memory() as f64 / 1024.0 / 1024.0;
                        total_threads += process.tasks().map(|t| t.len()).unwrap_or(1);
                        total_disk_read += process.disk_usage().total_read_bytes;
                        total_disk_write += process.disk_usage().total_written_bytes;
                    }
                }

                let timestamp = self.start_time.elapsed().as_secs_f64();

                let sample = ResourceSample {
                    timestamp,
                    cpu_usage: total_cpu,
                    memory_mb: total_memory,
                    thread_count: total_threads,
                    disk_read_bytes: total_disk_read,
                    disk_write_bytes: total_disk_write,
                };

                let load_avg = System::load_average().one;

                if let Ok(mut stats) = self.stats.lock() {
                    stats.load_avg_1min = load_avg;
                    stats.add_sample(sample);
                }
            }
        })
    }

    pub fn get_stats(&self) -> ResourceStats {
        self.stats.lock().unwrap().clone()
    }
}
