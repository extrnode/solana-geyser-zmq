use log::info;
use std::{fmt, sync::Arc, thread, time};

pub struct Metrics {
    pub send_errs: std::sync::atomic::AtomicUsize,
    pub tx_serialize_errs: std::sync::atomic::AtomicUsize,
    pub instruction_serialize_errs: std::sync::atomic::AtomicUsize,
    pub sender_lock_errs: std::sync::atomic::AtomicUsize,
    pub conn_lock_errs: std::sync::atomic::AtomicUsize,
    pub untyped_errs: std::sync::atomic::AtomicUsize,
}

impl Metrics {
    pub fn new_rc() -> Arc<Self> {
        Arc::new(Self {
            send_errs: std::sync::atomic::AtomicUsize::new(0),
            tx_serialize_errs: std::sync::atomic::AtomicUsize::new(0),
            instruction_serialize_errs: std::sync::atomic::AtomicUsize::new(0),
            sender_lock_errs: std::sync::atomic::AtomicUsize::new(0),
            conn_lock_errs: std::sync::atomic::AtomicUsize::new(0),
            untyped_errs: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    pub fn spin(&self, interval: time::Duration) {
        loop {
            info!("{}", self);
            thread::sleep(interval)
        }
    }
}

impl std::fmt::Display for Metrics {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("geyser-metrics")
            .field("send_errs", &self.send_errs)
            .field("tx_serialize_errs", &self.tx_serialize_errs)
            .field(
                "instruction_serialize_errs",
                &self.instruction_serialize_errs,
            )
            .field("sender_lock_errs", &self.sender_lock_errs)
            .field("conn_lock_errs", &self.conn_lock_errs)
            .field("untyped_errs", &self.untyped_errs)
            .finish()
    }
}
