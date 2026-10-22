use std::time::Instant;

/// Weight of the existing average per sample kept sluggish because downloads are bursty
const SMOOTHING: f64 = 0.7;

#[derive(Debug, Clone, Copy, Default)]
pub struct TransferMeter {
    last_sample: Option<(Instant, u64)>,
    speed_bps: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransferStats {
    pub speed_bps: f64,
    /// `None` until there is enough signal to estimate or when the total is unknown
    pub eta_secs: Option<u64>,
}

impl TransferMeter {
    /// `completed` and `total` are cumulative bytes
    /// Samples under 250ms apart are not folded in
    /// the tiny elapsed divisor would swamp the average with noise
    pub fn sample(&mut self, completed: u64, total: u64) -> Option<TransferStats> {
        let now = Instant::now();

        let Some((last_at, last_completed)) = self.last_sample else {
            self.last_sample = Some((now, completed));
            return None;
        };

        let elapsed = now.duration_since(last_at).as_secs_f64();
        if elapsed < 0.25 {
            return self.stats(completed, total);
        }

        let delta = completed.saturating_sub(last_completed) as f64;
        let instant = delta / elapsed;

        self.speed_bps = if self.speed_bps <= 0.0 {
            instant
        } else {
            self.speed_bps * SMOOTHING + instant * (1.0 - SMOOTHING)
        };
        self.last_sample = Some((now, completed));

        self.stats(completed, total)
    }

    fn stats(&self, completed: u64, total: u64) -> Option<TransferStats> {
        if self.speed_bps <= 0.0 {
            return None;
        }

        let remaining = total.saturating_sub(completed);
        Some(TransferStats {
            speed_bps: self.speed_bps,
            // No total means no estimate the denominator may still be climbing
            eta_secs: (total > 0).then(|| (remaining as f64 / self.speed_bps) as u64),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_sample_only_establishes_a_baseline() {
        let mut meter = TransferMeter::default();
        assert!(meter.sample(0, 1000).is_none());
    }

    #[test]
    fn a_total_of_zero_yields_speed_without_an_eta() {
        let mut meter = TransferMeter::default();
        meter.sample(0, 0);
        std::thread::sleep(std::time::Duration::from_millis(300));

        let stats = meter.sample(1_000, 0).expect("should have a rate by now");
        assert!(stats.speed_bps > 0.0);
        assert_eq!(stats.eta_secs, None);
    }

    #[test]
    fn eta_falls_as_the_download_completes() {
        let mut meter = TransferMeter::default();
        meter.sample(0, 10_000);
        std::thread::sleep(std::time::Duration::from_millis(300));
        let early = meter.sample(1_000, 10_000).unwrap().eta_secs.unwrap();

        std::thread::sleep(std::time::Duration::from_millis(300));
        let late = meter.sample(9_000, 10_000).unwrap().eta_secs.unwrap();

        assert!(late < early, "eta should shrink: {early} -> {late}");
    }

    #[test]
    fn samples_taken_too_close_together_do_not_distort_the_average() {
        let mut meter = TransferMeter::default();
        meter.sample(0, 10_000);
        std::thread::sleep(std::time::Duration::from_millis(300));
        let settled = meter.sample(1_000, 10_000).unwrap().speed_bps;

        for i in 1..50 {
            meter.sample(1_000 + i, 10_000);
        }

        let after = meter.speed_bps;
        assert!(
            (after - settled).abs() < f64::EPSILON,
            "burst of close samples changed the rate: {settled} -> {after}"
        );
    }
}
