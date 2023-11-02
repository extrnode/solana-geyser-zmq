use std::{fmt, sync::Arc};

pub struct Metrics {
    pub send_errs: std::sync::atomic::AtomicU64,
    pub disconnect_errs: std::sync::atomic::AtomicU64,
    pub serialize_errs: std::sync::atomic::AtomicU64,
    pub sender_lock_errs: std::sync::atomic::AtomicU64,
    pub conn_lock_errs: std::sync::atomic::AtomicU64,
    pub cache_lock_errs: std::sync::atomic::AtomicU64,
    pub untyped_errs: std::sync::atomic::AtomicU64,
}

impl Metrics {
    pub fn new_rc() -> Arc<Self> {
        Arc::new(Self {
            send_errs: std::sync::atomic::AtomicU64::new(0),
            disconnect_errs: std::sync::atomic::AtomicU64::new(0),
            serialize_errs: std::sync::atomic::AtomicU64::new(0),
            sender_lock_errs: std::sync::atomic::AtomicU64::new(0),
            conn_lock_errs: std::sync::atomic::AtomicU64::new(0),
            cache_lock_errs: std::sync::atomic::AtomicU64::new(0),
            untyped_errs: std::sync::atomic::AtomicU64::new(0),
        })
    }
}

impl std::fmt::Display for Metrics {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("geyser-metrics")
            .field("send_errs", &self.send_errs)
            .field("disconnect_errs", &self.disconnect_errs)
            .field("serialize_errs", &self.serialize_errs)
            .field("sender_lock_errs", &self.sender_lock_errs)
            .field("conn_lock_errs", &self.conn_lock_errs)
            .field("cache_lock_errs", &self.cache_lock_errs)
            .field("untyped_errs", &self.untyped_errs)
            .finish()
    }
}
