//! What the machine can throw at the game, queried once and cached

/// Logical CPUs, the same count a JVM reports as available processors, so
/// launcher heuristics can line up with the ones the client ships
#[must_use]
pub fn total_cpu_cores() -> u32 {
    static CORES: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

    *CORES
        .get_or_init(|| std::thread::available_parallelism().map_or(1, |cores| cores.get() as u32))
}

#[cfg(test)]
mod tests {
    use super::total_cpu_cores;

    #[test]
    fn the_core_count_is_at_least_one() {
        assert!(total_cpu_cores() >= 1);
    }
}
