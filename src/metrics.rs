use std::collections::{HashMap, HashSet, VecDeque};
use sysinfo::{Pid, ProcessesToUpdate, System};

pub struct ProcessMetrics {
    system: System,
}

impl ProcessMetrics {
    pub fn new() -> Self {
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::All, true);
        Self { system }
    }

    pub fn refresh(&mut self) {
        self.system.refresh_processes(ProcessesToUpdate::All, true);
    }

    pub fn query_tree(&self, root_pid: i32) -> (Option<f32>, Option<u64>) {
        if root_pid <= 0 {
            return (None, None);
        }

        let root_sys_pid = Pid::from_u32(root_pid as u32);
        if !self.system.processes().contains_key(&root_sys_pid) {
            return (None, None);
        }

        // Build parent -> children relationship map
        let mut children_map: HashMap<Pid, Vec<Pid>> = HashMap::new();
        for (pid, proc) in self.system.processes() {
            if let Some(parent_pid) = proc.parent() {
                children_map.entry(parent_pid).or_default().push(*pid);
            }
        }

        // Traverse the entire process tree starting from root_sys_pid
        let mut total_cpu = 0.0f32;
        let mut total_mem_bytes = 0u64;
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(root_sys_pid);
        visited.insert(root_sys_pid);

        while let Some(current_pid) = queue.pop_front() {
            if let Some(proc) = self.system.process(current_pid) {
                total_cpu += proc.cpu_usage();
                total_mem_bytes += proc.memory();
            }

            if let Some(children) = children_map.get(&current_pid) {
                for child in children {
                    if visited.insert(*child) {
                        queue.push_back(*child);
                    }
                }
            }
        }

        let mem_mb = if total_mem_bytes == 0 {
            0
        } else {
            (total_mem_bytes / (1024 * 1024)).max(1)
        };

        (Some(total_cpu), Some(mem_mb))
    }
}

impl Default for ProcessMetrics {
    fn default() -> Self {
        Self::new()
    }
}
