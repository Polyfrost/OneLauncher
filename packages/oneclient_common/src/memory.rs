use sysinfo::{MemoryRefreshKind, RefreshKind, System};

pub const MEMORY_HEADROOM_GB: u32 = 2;

const EIGHT_GB_MB: u32 = 7 * 1024;
const TWELVE_GB_MB: u32 = 11 * 1024;

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
    match total_mb {
        0..EIGHT_GB_MB => 2048,
        EIGHT_GB_MB..TWELVE_GB_MB => 3072,
        _ => 4096,
    }
}

#[cfg(test)]
mod tests {
    use super::default_mem_max_for_total;

    #[test]
    fn the_default_heap_ramps_with_total_ram() {
        assert_eq!(default_mem_max_for_total(3987), 2048); // 4GB
        assert_eq!(default_mem_max_for_total(6060), 2048); // 6GB
        assert_eq!(default_mem_max_for_total(7167), 2048);
        assert_eq!(default_mem_max_for_total(7168), 3072);
        assert_eq!(default_mem_max_for_total(7900), 3072); // 8GB
        assert_eq!(default_mem_max_for_total(11263), 3072);
        assert_eq!(default_mem_max_for_total(11264), 4096);
        assert_eq!(default_mem_max_for_total(11800), 4096); // 12GB
        assert_eq!(default_mem_max_for_total(16290), 4096); // 16GB
        assert_eq!(default_mem_max_for_total(65229), 4096); // 64GB
    }

    #[test]
    fn a_machine_that_reports_nothing_still_gets_a_heap() {
        assert_eq!(default_mem_max_for_total(0), 2048);
    }
}
