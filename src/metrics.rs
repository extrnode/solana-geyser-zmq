use log::info;
use std::{fmt, sync::Arc, thread, time};

pub struct Metrics {
    pub send_errs: std::sync::atomic::AtomicUsize,
    pub errs: std::sync::atomic::AtomicUsize,
}

impl Metrics {
    pub fn new_rc() -> Arc<Self> {
        Arc::new(Self {
            send_errs: std::sync::atomic::AtomicUsize::new(0),
            errs: std::sync::atomic::AtomicUsize::new(0),
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
        fmt.debug_struct("geyser-zmq")
            .field("errs", &self.errs)
            .field("send_errs", &self.send_errs)
            .finish()
    }
}
