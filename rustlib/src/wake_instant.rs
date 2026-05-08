use std::ops::Add;
use std::time::Duration;

/// A monotonic point in time, on a clock that continues to advance while the system is asleep, hibernating, or otherwise suspended.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct WakeInstant {
    nanos: u64,
}

impl WakeInstant {
    pub fn now() -> Self {
        Self { nanos: now_nanos() }
    }

    pub fn checked_duration_since(self, earlier: Self) -> Option<Duration> {
        self.nanos.checked_sub(earlier.nanos).map(Duration::from_nanos)
    }

    /// How much time until a future WakeInstant
    pub fn remaining(self) -> Duration {
        self.saturating_duration_since(Self::now())
    }

    pub fn saturating_duration_since(self, earlier: Self) -> Duration {
        Duration::from_nanos(self.nanos.saturating_sub(earlier.nanos))
    }
}

impl Add<Duration> for WakeInstant {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        let rhs_nanos = u64::try_from(rhs.as_nanos()).expect("Duration too large to add to WakeInstant");
        Self { nanos: self.nanos.checked_add(rhs_nanos).expect("WakeInstant overflow") }
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn now_nanos() -> u64 {
    unsafe extern "C" {
        fn clock_gettime_nsec_np(clk_id: libc::clockid_t) -> u64;
    }
    // SAFETY: no preconditions; returns 0 only for an invalid clock id.
    unsafe { clock_gettime_nsec_np(libc::CLOCK_MONOTONIC_RAW) }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
fn now_nanos() -> u64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: writes into a stack-allocated timespec.
    let res = unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut ts) };
    assert_eq!(res, 0, "clock_gettime(CLOCK_BOOTTIME) failed: {}", std::io::Error::last_os_error());
    u64::try_from(ts.tv_sec).expect("CLOCK_BOOTTIME seconds overflow") * 1_000_000_000
        + u64::try_from(ts.tv_nsec).expect("CLOCK_BOOTTIME nanoseconds overflow")
}

#[cfg(target_os = "windows")]
fn now_nanos() -> u64 {
    // SAFETY: no preconditions.
    let hundred_nanos = unsafe { windows::Win32::System::WindowsProgramming::QueryInterruptTimePrecise() };
    hundred_nanos * 100
}
