use geoforge_protocol::{ExecutionOptions, ResolvedExecutionOptions};

pub struct ResourceBudget {
    pub resolved: ResolvedExecutionOptions,
}

impl ResourceBudget {
    pub fn new(options: &ExecutionOptions) -> Self {
        Self {
            resolved: options.resolve(),
        }
    }

    pub fn cpu_workers(&self) -> u32 {
        self.resolved.cpu_workers
    }

    pub fn memory_budget_mib(&self) -> u64 {
        self.resolved.memory_budget_mib
    }

    pub fn io_workers(&self) -> u32 {
        self.resolved.io_workers
    }

    pub fn convert_threads(&self) -> u32 {
        self.resolved.cpu_workers
    }

    pub fn rebuild_workers(&self) -> u32 {
        self.resolved.cpu_workers
    }

    pub fn texture_file_workers(&self) -> u32 {
        self.resolved.io_workers.min(2)
    }

    pub fn texture_encoder_threads(&self) -> u32 {
        let total_cpu = self.resolved.cpu_workers;
        let file_workers = self.texture_file_workers();
        (total_cpu / file_workers.max(1)).max(1).min(4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geoforge_protocol::CpuWorkers;

    #[test]
    fn budget_from_auto() {
        let opts = ExecutionOptions::default();
        let budget = ResourceBudget::new(&opts);
        assert!(budget.cpu_workers() >= 1);
        assert!(budget.memory_budget_mib() > 0);
        assert!(budget.io_workers() >= 1);
    }

    #[test]
    fn budget_from_explicit() {
        let opts = ExecutionOptions {
            cpu_workers: CpuWorkers::Count(4),
            memory_budget_mib: Some(8192),
            io_workers: Some(2),
        };
        let budget = ResourceBudget::new(&opts);
        assert_eq!(budget.cpu_workers(), 4);
        assert_eq!(budget.memory_budget_mib(), 8192);
        assert_eq!(budget.io_workers(), 2);
    }

    #[test]
    fn convert_threads_uses_cpu_workers() {
        let opts = ExecutionOptions {
            cpu_workers: CpuWorkers::Count(4),
            memory_budget_mib: None,
            io_workers: None,
        };
        let budget = ResourceBudget::new(&opts);
        assert_eq!(budget.convert_threads(), 4);
    }

    #[test]
    fn rebuild_workers_uses_cpu_workers() {
        let opts = ExecutionOptions {
            cpu_workers: CpuWorkers::Count(8),
            memory_budget_mib: None,
            io_workers: None,
        };
        let budget = ResourceBudget::new(&opts);
        assert_eq!(budget.rebuild_workers(), 8);
    }

    #[test]
    fn texture_workers_conservative() {
        let opts = ExecutionOptions {
            cpu_workers: CpuWorkers::Count(8),
            memory_budget_mib: None,
            io_workers: Some(8),
        };
        let budget = ResourceBudget::new(&opts);
        assert_eq!(budget.texture_file_workers(), 2);
    }
}
