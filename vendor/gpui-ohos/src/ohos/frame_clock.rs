use std::time::{Duration, Instant};

/// Maps NativeVSync's CLOCK_MONOTONIC nanoseconds to the Instant clock used by
/// input dispatch. The fixed origin preserves VSync intervals even when either
/// the native callback or the Ability main queue runs late.
#[derive(Clone, Copy)]
pub(crate) struct FrameClock {
    monotonic_origin: Duration,
    instant_origin: Instant,
}

impl FrameClock {
    pub(crate) fn new() -> Option<Self> {
        let before = Instant::now();
        let mut time = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: `time` is a valid writable timespec; CLOCK_MONOTONIC is the
        // clock documented for OH_NativeVSync_FrameCallback timestamps.
        if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) } != 0 {
            return None;
        }
        let after = Instant::now();
        Some(Self {
            monotonic_origin: Duration::new(
                time.tv_sec.try_into().ok()?,
                time.tv_nsec.try_into().ok()?,
            ),
            instant_origin: before + after.duration_since(before) / 2,
        })
    }

    pub(crate) fn instant_for_timestamp(&self, timestamp_ns: i64) -> Option<Instant> {
        if timestamp_ns <= 0 {
            return None;
        }
        let timestamp = Duration::from_nanos(timestamp_ns as u64);
        if timestamp >= self.monotonic_origin {
            self.instant_origin
                .checked_add(timestamp - self.monotonic_origin)
        } else {
            self.instant_origin
                .checked_sub(self.monotonic_origin - timestamp)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_timestamps_keep_their_intervals_and_input_clock_origin() {
        let origin = Instant::now();
        let clock = FrameClock {
            monotonic_origin: Duration::from_secs(10),
            instant_origin: origin,
        };
        assert_eq!(
            clock.instant_for_timestamp(9_990_000_000),
            origin.checked_sub(Duration::from_millis(10))
        );
        assert_eq!(
            clock.instant_for_timestamp(10_008_333_333),
            Some(origin + Duration::from_nanos(8_333_333))
        );
        assert_eq!(
            clock.instant_for_timestamp(10_016_666_666),
            Some(origin + Duration::from_nanos(16_666_666))
        );
        assert_eq!(clock.instant_for_timestamp(0), None);
        assert_eq!(clock.instant_for_timestamp(-1), None);
    }

    #[test]
    fn calibration_maps_the_native_clock_into_the_current_instant_range() {
        let before = Instant::now();
        let clock = FrameClock::new().unwrap();
        let mapped = clock
            .instant_for_timestamp(clock.monotonic_origin.as_nanos() as i64)
            .unwrap();
        let after = Instant::now();
        assert!(mapped >= before && mapped <= after);
    }
}
