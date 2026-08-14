use objc::{msg_send, sel, sel_impl};
use serde::{Deserialize, Serialize};

/// Stable application identity combining PID and launch date.
///
/// Prevents PID-reuse races: after an app terminates the kernel may reuse
/// its PID for a different process. Binding ownership to `(pid, launch_date)`
/// ensures we never restore a wrong process that inherited the same PID.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct AppIdentity {
    pub pid: i32,
    pub launch_date: LaunchDateBits,
}

/// Exact target for every delayed, cached, or external window operation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct WindowTarget {
    pub identity: AppIdentity,
    pub window_id: u32,
}

/// High-precision launch-date bits from
/// `NSRunningApplication.launchDate.timeIntervalSinceReferenceDate`.
///
/// Stored as `f64::to_bits` for `Send + Sync + Copy`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct LaunchDateBits(u64);

impl LaunchDateBits {
    /// Creates bits from a `timeIntervalSinceReferenceDate` value.
    /// Returns `None` if `t` is not finite and >0 (fail-closed on
    /// missing/null launch date).
    #[must_use]
    pub const fn from_time_interval_since_reference_date(t: f64) -> Option<Self> {
        if t.is_finite() && t > 0.0 {
            Some(Self(t.to_bits()))
        } else {
            None
        }
    }

    /// Returns the stored launch-date bit pattern for structured diagnostics.
    #[must_use]
    pub const fn bits(self) -> u64 { self.0 }
}

impl AppIdentity {
    /// Captures identity from an `NSRunningApplication` `ObjC` object.
    ///
    /// Returns `None` if pid ≤ 0, launchDate is null, or the time interval is
    /// not finite and positive. This is fail-closed: callers must skip the app
    /// rather than proceeding with an invalid identity.
    ///
    /// # Safety
    ///
    /// `app` must be a valid non-null `*mut Object` pointing to an
    /// `NSRunningApplication` instance for the duration of this call.
    #[must_use]
    pub unsafe fn from_ns_running_app(app: *mut objc::runtime::Object) -> Option<Self> {
        unsafe {
            if app.is_null() {
                return None;
            }
            let pid: i32 = msg_send![app, processIdentifier];
            if pid <= 0 {
                return None;
            }
            let launch_date: *mut objc::runtime::Object = msg_send![app, launchDate];
            if launch_date.is_null() {
                return None;
            }
            let interval: f64 = msg_send![launch_date, timeIntervalSinceReferenceDate];
            let bits = LaunchDateBits::from_time_interval_since_reference_date(interval)?;
            Some(Self { pid, launch_date: bits })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_from_null_fails_closed() {
        let identity = unsafe { AppIdentity::from_ns_running_app(std::ptr::null_mut()) };
        assert!(identity.is_none());
    }

    #[test]
    fn identity_ord_deterministic() {
        let a = AppIdentity {
            pid: 10,
            launch_date: LaunchDateBits::from_time_interval_since_reference_date(1000.0).unwrap(),
        };
        let b = AppIdentity {
            pid: 10,
            launch_date: LaunchDateBits::from_time_interval_since_reference_date(2000.0).unwrap(),
        };
        let c = AppIdentity {
            pid: 20,
            launch_date: LaunchDateBits::from_time_interval_since_reference_date(1000.0).unwrap(),
        };
        assert!(a < b);
        assert!(a < c);
        assert!(b < c);
    }

    #[test]
    fn identity_equality_pid_and_launch_date() {
        let a = AppIdentity {
            pid: 10,
            launch_date: LaunchDateBits::from_time_interval_since_reference_date(42.0).unwrap(),
        };
        let b = AppIdentity {
            pid: 10,
            launch_date: LaunchDateBits::from_time_interval_since_reference_date(42.0).unwrap(),
        };
        let c = AppIdentity {
            pid: 10,
            launch_date: LaunchDateBits::from_time_interval_since_reference_date(43.0).unwrap(),
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn launch_date_bits_rejects_non_positive_and_non_finite() {
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(LaunchDateBits::from_time_interval_since_reference_date(bad).is_none());
        }
    }

    #[test]
    fn window_target_distinguishes_same_window_id() {
        let a = AppIdentity {
            pid: 10,
            launch_date: LaunchDateBits::from_time_interval_since_reference_date(42.0).unwrap(),
        };
        let b = AppIdentity {
            pid: 10,
            launch_date: LaunchDateBits::from_time_interval_since_reference_date(43.0).unwrap(),
        };
        assert_ne!(WindowTarget { identity: a, window_id: 99 }, WindowTarget {
            identity: b,
            window_id: 99
        },);
    }
}
