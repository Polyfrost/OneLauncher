use sysinfo::{MemoryRefreshKind, RefreshKind, System};

const SMALL_SYSTEM_MB: u32 = 7 * 1024;

const SMALL_SYSTEM_HEAP_MB: u32 = 2048;
const DEFAULT_HEAP_MB: u32 = 4096;

pub fn total_ram_mb() -> u32 {
    static TOTAL_RAM_MB: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

    *TOTAL_RAM_MB.get_or_init(|| {
        let system = System::new_with_specifics(
            RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()),
        );
        (system.total_memory() / 1024 / 1024).min(u32::MAX as u64) as u32
    })
}

#[must_use]
pub fn default_mem_max() -> u32 {
    default_mem_max_for_total(total_ram_mb())
}

#[must_use]
pub fn default_mem_max_for_total(total_mb: u32) -> u32 {
    if total_mb < SMALL_SYSTEM_MB {
        SMALL_SYSTEM_HEAP_MB
    } else {
        DEFAULT_HEAP_MB
    }
}

#[cfg(test)]
mod tests {
    use super::default_mem_max_for_total;

    #[test]
    fn small_systems_get_2gb() {
        assert_eq!(default_mem_max_for_total(3987), 2048); // 4GB
        assert_eq!(default_mem_max_for_total(6060), 2048); // 6GB
        assert_eq!(default_mem_max_for_total(7167), 2048);
        assert_eq!(default_mem_max_for_total(7168), 4096); // 8GB
        assert_eq!(default_mem_max_for_total(16290), 4096); // 16GB
    }
}
