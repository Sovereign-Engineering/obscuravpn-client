#[macro_export]
macro_rules! rate_limited_log {
    ($silence:expr, $($log:tt)*) => {{
        static LAST_LOG_AT: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
        let now = std::time::Instant::now();
        let mut last = LAST_LOG_AT.lock().unwrap();
        if !last.is_some_and(|last| last + $silence > now) {
            *last = Some(now);
            drop(last);
            $($log)*
        }
    }};
}
