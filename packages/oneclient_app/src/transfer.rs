//! Download speed and time-remaining estimation.
//!
//! One shared estimator, so the notification engine and the onboarding
//! installer show the same numbers arrived at the same way.

use std::time::Instant;

/// Weight given to the existing average when a new sample arrives.
///
/// Downloads are bursty (a single asset finishing can spike the instantaneous
/// rate several-fold) so the average is kept sluggish on purpose.
const SMOOTHING: f64 = 0.7;

/// An exponentially-weighted average of transfer rate, plus the ETA that falls
/// out of it.
#[derive(Debug, Clone, Copy, Default)]
pub struct TransferMeter {
    /// Timestamp and byte count of the last sample.
    last_sample: Option<(Instant, u64)>,
    speed_bps: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransferStats {
    pub speed_bps: f64,
    /// `None` until there is enough signal to estimate, or when the total is
    /// unknown.
    pub eta_secs: Option<u64>,
}

impl TransferMeter {
    /// Records progress and returns the current estimate.
    ///
    /// `completed` and `total` are cumulative byte counts. Samples closer
    /// together than a few hundred milliseconds are ignored rather than folded
    /// in: the elapsed divisor gets small enough to swamp the average with
    /// noise, which is what the fixed-tick version was working around by
    /// sampling on a timer instead.
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
            // No total means no estimate, which beats a countdown derived from a
            // denominator that is still climbing.
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

    /// Rapid updates are the common case during an asset fan-out; they must not
    /// divide by a near-zero elapsed and blow the average up.
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
