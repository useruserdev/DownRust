// SPDX-License-Identifier: GPL-3.0-or-later
//! Speed measurement with an exponential moving average.
use std::time::Duration;

use tokio::time::Instant;

const MIN_INTERVAL: Duration = Duration::from_millis(50);

/// Smooths the byte count over time into a speed in bytes per second.
#[derive(Debug, Default)]
pub(crate) struct SpeedMeter {
    last_bytes: u64,
    last_at: Option<Instant>,
    ema: f64,
}

impl SpeedMeter {
    /// Feeds the running byte total and returns the smoothed speed.
    pub fn sample(&mut self, total: u64, now: Instant) -> u64 {
        let Some(last_at) = self.last_at else {
            self.last_at = Some(now);
            self.last_bytes = total;
            return 0;
        };
        let dt = now.saturating_duration_since(last_at);
        if dt < MIN_INTERVAL {
            return self.ema as u64;
        }
        let inst = total.saturating_sub(self.last_bytes) as f64 / dt.as_secs_f64();
        self.ema = if self.ema == 0.0 {
            inst
        } else {
            0.3 * inst + 0.7 * self.ema
        };
        self.last_at = Some(now);
        self.last_bytes = total;
        self.ema as u64
    }
}

/// Seconds left at the given speed. `None` when the speed is 0.
pub(crate) fn eta_secs(remaining: u64, speed: u64) -> Option<u64> {
    if speed == 0 {
        None
    } else {
        Some(remaining.div_ceil(speed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sample_is_zero_and_fast_samples_return_ema() {
        let t0 = Instant::now();
        let mut m = SpeedMeter::default();
        assert_eq!(m.sample(0, t0), 0);
        let t1 = t0 + Duration::from_secs(1);
        assert_eq!(m.sample(1000, t1), 1000);
        assert_eq!(m.sample(9999, t1 + Duration::from_millis(10)), 1000);
    }

    #[test]
    fn ema_math_follows_formula() {
        let t0 = Instant::now();
        let mut m = SpeedMeter::default();
        m.sample(0, t0);
        assert_eq!(m.sample(1000, t0 + Duration::from_secs(1)), 1000);
        let v = m.sample(3000, t0 + Duration::from_secs(2));
        assert!((1299..=1300).contains(&v), "ema {v}");
    }

    #[test]
    fn eta_handles_zero_speed_and_rounds_up() {
        assert_eq!(eta_secs(1000, 300), Some(4));
        assert_eq!(eta_secs(0, 10), Some(0));
        assert_eq!(eta_secs(5, 0), None);
    }
}
