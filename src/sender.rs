use log::{error, info};
use std::io::{self, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, RwLock};
use std::thread;

use crate::errors::GeyserError;

pub struct TcpSender {
    conns: Arc<RwLock<Vec<SyncSender<Vec<u8>>>>>,
}

impl TcpSender {
    pub fn default() -> Self {
        TcpSender {
            conns: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn pack_message(&self, msg: Vec<u8>) -> Vec<u8> {
        const HEADER_BYTE_SIZE: usize = 4;
        let mut result = Vec::new();
        result.reserve_exact(HEADER_BYTE_SIZE + msg.len());

        result.extend_from_slice(&(msg.len() as u32).to_le_bytes());
        result.extend_from_slice(&msg);

        result
    }

    pub fn publish(&self, message: Vec<u8>) -> Result<(), GeyserError> {
        let mut conns_to_remove = Vec::new();
        let mut send_errs = 0;
        let message = self.pack_message(message);

        {
            let conns = self
                .conns
                .read()
                .map_err(|_| GeyserError::SenderLockError)?;

            for (i, conn) in conns.iter().enumerate() {
                if let Err(e) = conn.try_send(message.clone()) {
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
                    Ok(stream) => {
                        let conns = conns.clone();
                        let (tx, rx) = sync_channel(buffer_size);
                        if Self::add_conn(&conns, tx).is_err() {
                            continue;
                        }

                        thread::spawn(move || {
                            handle_connection(stream, rx);
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

fn handle_connection(mut stream: TcpStream, rx: Receiver<Vec<u8>>) {
    for msg in rx {
        if let Err(e) = stream.write_all(&msg) {
            error!("Error writing data: {}", e);
            break;
        }
    }
}
