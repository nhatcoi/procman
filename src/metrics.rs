use sysinfo::{Pid, ProcessesToUpdate, System};

pub struct ProcessMetrics {
    system: System,
}

impl ProcessMetrics {
    pub fn new() -> Self {
        Self {
            system: System::new(),
        }
    }

    pub fn query(&mut self, pid: i32) -> (Option<f32>, Option<u64>) {
        if pid <= 0 {
            return (None, None);
        }

        let sys_pid = Pid::from_u32(pid as u32);
        self.system.refresh_processes(ProcessesToUpdate::Some(&[sys_pid]), true);

        if let Some(proc) = self.system.process(sys_pid) {
            let cpu = proc.cpu_usage();
            let mem_mb = proc.memory() / (1024 * 1024);
            (Some(cpu), Some(mem_mb))
        } else {
            (None, None)
        }
    }
}
