use core::time;
use log::{error, info, warn};
use std::io::{self, Write};
use std::net::TcpListener;
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use crate::errors::GeyserError;

const DEFAULT_VECTOR_PREALLOC: usize = 1024 * 1024;
pub const HEADER_BYTE_SIZE: usize = 4;

pub struct TcpBuffer {
    data: Vec<Vec<u8>>,
    total_bytesize: usize,
}

impl TcpBuffer {
    pub fn append(&mut self, msg: Vec<u8>) {
        let mut result = Vec::with_capacity(HEADER_BYTE_SIZE + msg.len());
        result.extend_from_slice(&(msg.len() as u32).to_le_bytes());
        result.extend_from_slice(&msg);

        self.total_bytesize += result.len();
        self.data.push(result);
    }

    pub fn flush_data(&mut self) -> Vec<u8> {
        let mut batch = Vec::with_capacity(HEADER_BYTE_SIZE + self.total_bytesize);
        batch.extend_from_slice(&(self.total_bytesize as u32).to_le_bytes());
        self.data.iter().for_each(|msg| {
            batch.extend_from_slice(msg);
        });

        // Clear buffers
        self.data.clear();
        self.total_bytesize = 0;

        batch
    }
}

pub struct TcpSender {
    batch_max_bytes: usize,
    strict_delivery: bool,
    min_subscribers: usize,
    conns: Arc<RwLock<Vec<SyncSender<Vec<u8>>>>>,
    buffer: Mutex<TcpBuffer>,
}

impl TcpSender {
    pub fn new(batch_max_bytes: usize, strict_delivery: bool, min_subscribers: usize) -> Self {
        TcpSender {
            batch_max_bytes,
            strict_delivery,
            min_subscribers,
            conns: Arc::new(RwLock::new(Vec::new())),
            buffer: Mutex::new(TcpBuffer {
                data: Vec::with_capacity(DEFAULT_VECTOR_PREALLOC),
                total_bytesize: 0,
            }),
        }
    }

    pub fn publish(&self, message: Vec<u8>) -> Result<(), GeyserError> {
        let mut buffer = self
            .buffer
            .lock()
            .map_err(|_| GeyserError::SenderLockError)?;

        buffer.append(message);

        if buffer.total_bytesize < self.batch_max_bytes {
            return Ok(());
        }

        loop {
            if let Err(e) = self.publish_batch(buffer.flush_data()) {
                if self.strict_delivery {
                    // for strict delivery, try_send until there's no error
                    continue;
                } else {
                    // for regular mode just return error
                    return Err(e);
                }
            }

            break;
        }

        Ok(())
    }

    pub fn wait_min_subscribers(&self) -> Result<(), GeyserError> {
        if self.min_subscribers > 0 {
            loop {
                let conns = {
                    let conns = self
                        .conns
                        .read()
                        .map_err(|_| GeyserError::SenderLockError)?;

                    conns.len()
                };

                if conns >= self.min_subscribers {
                    break;
                }

                warn!("not enough subscribers {}/{}", conns, self.min_subscribers);

                thread::sleep(time::Duration::from_secs(1));
            }
        }

        Ok(())
    }

    pub fn publish_batch(&self, batch: Vec<u8>) -> Result<(), GeyserError> {
        let mut conns_to_remove = Vec::new();
        let mut send_errs = 0;
        let mut disconnects = 0;

        self.wait_min_subscribers()?;

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
                            disconnects += 1;
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

        if disconnects > 0 {
            return Err(GeyserError::TcpDisconnects(disconnects));
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

#[cfg(test)]
mod tests {
    use super::TcpSender;
    use super::*;
    use crate::receiver::TcpReceiver;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_sender() {
        let sender = TcpSender::new(10, false, 0);
        sender.bind(9050, 100).unwrap();

        let received_count = Arc::new(Mutex::new(0));
        let received_count_clone = received_count.clone();

        thread::spawn(move || {
            let receiver = TcpReceiver::new(
                Box::new(move |_data| {
                    let mut received_count = received_count_clone.lock().unwrap();
                    *received_count += 1;
                }),
                Duration::from_secs(1),
                Duration::from_secs(1),
            );
            receiver.connect("127.0.0.1:9050".parse().unwrap()).unwrap();
        });

        sleep(Duration::from_secs(1));

        let sent_messages = 100;
        let msg = b"hello world".to_vec();
        for _ in 0..sent_messages {
            sender.publish(msg.clone()).unwrap();
        }

        sleep(Duration::from_secs(2));

        assert_eq!(sent_messages, *received_count.lock().unwrap());
    }
}
