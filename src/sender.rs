use log::{error, info};
use std::io::{self, Write};
use std::net::TcpListener;
use std::sync::atomic::AtomicUsize;
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use crate::errors::GeyserError;

const DEFAULT_VECTOR_PREALLOC: usize = 1024 * 1024;
const HEADER_BYTE_SIZE: usize = 4;

pub struct TcpSender {
    batch_max_bytes: usize,
    conns: Arc<RwLock<Vec<SyncSender<Vec<u8>>>>>,
    buffer: Mutex<Vec<Vec<u8>>>,
    total_bytesize: AtomicUsize,
}

impl TcpSender {
    pub fn new(batch_max_bytes: usize) -> Self {
        TcpSender {
            batch_max_bytes,
            conns: Arc::new(RwLock::new(Vec::new())),
            buffer: Mutex::new(Vec::with_capacity(DEFAULT_VECTOR_PREALLOC)),
            total_bytesize: AtomicUsize::new(0),
        }
    }

    pub fn publish(&self, message: Vec<u8>) -> Result<(), GeyserError> {
        let message = pack_message(message);
        let message_len = message.len();

        let mut buffer = self
            .buffer
            .lock()
            .map_err(|_| GeyserError::SenderLockError)?;

        buffer.push(message);

        let total_bytesize = self
            .total_bytesize
            .fetch_add(message_len, std::sync::atomic::Ordering::Relaxed);

        if total_bytesize < self.batch_max_bytes {
            return Ok(());
        }

        let mut batch = Vec::with_capacity(total_bytesize + HEADER_BYTE_SIZE);
        batch.extend_from_slice(&(total_bytesize as u32).to_le_bytes());
        buffer.iter().for_each(|msg| {
            batch.extend_from_slice(msg);
        });

        // Clear buffers
        buffer.clear();
        self.total_bytesize
            .store(0, std::sync::atomic::Ordering::Relaxed);

        self.publish_batch(batch)
    }

    pub fn publish_batch(&self, batch: Vec<u8>) -> Result<(), GeyserError> {
        let mut conns_to_remove = Vec::new();
        let mut send_errs = 0;

        {
            let conns = self
                .conns
                .read()
                .map_err(|_| GeyserError::SenderLockError)?;

            for (i, conn) in conns.iter().enumerate() {
                if let Err(e) = conn.try_send(batch.clone()) {
                    match e {
                        TrySendError::Full(..) => {
                            send_errs += 1;
                        }
                        _ => {
                            conns_to_remove.push(i);
                        }
                    }
                }
            }
        }

        if !conns_to_remove.is_empty() {
            let mut conns = self
                .conns
                .write()
                .map_err(|_| GeyserError::SenderLockError)?;

            conns_to_remove.iter().rev().for_each(|&i| {
                conns.remove(i);
            });
        }

        if send_errs > 0 {
            return Err(GeyserError::TcpSend(send_errs));
        }

        Ok(())
    }

    pub fn bind(&self, port: u16, buffer_size: usize) -> io::Result<()> {
        let listener = TcpListener::bind(("0.0.0.0", port))?;

        info!("TCP server listening on port {}", port);

        let conns = self.conns.clone();

        thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(mut stream) => {
                        let conns = conns.clone();
                        let (tx, rx) = sync_channel(buffer_size);
                        if Self::add_conn(&conns, tx).is_err() {
                            continue;
                        }

                        thread::spawn(move || {
                            for batch in rx {
                                if let Err(e) = stream.write_all(&batch) {
                                    error!("Error writing data: {}", e);
                                    break;
                                }
                            }
                        });
                    }
                    Err(e) => {
                        error!("Error accepting connection: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    fn add_conn(
        conns: &Arc<RwLock<Vec<SyncSender<Vec<u8>>>>>,
        conn: SyncSender<Vec<u8>>,
    ) -> Result<(), GeyserError> {
        let mut conns = conns.write().map_err(|_| GeyserError::ConnLockError)?;
        conns.push(conn);
        Ok(())
    }
}

fn pack_message(msg: Vec<u8>) -> Vec<u8> {
    let mut result = Vec::with_capacity(HEADER_BYTE_SIZE + msg.len());
    result.extend_from_slice(&(msg.len() as u32).to_le_bytes());
    result.extend_from_slice(&msg);

    result
}
