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

    pub fn publish(&self, message: Vec<u8>) -> Result<(), GeyserError> {
        let mut conns_to_remove = Vec::new();
        let mut send_errs = 0;

        {
            let conns = self.conns.read().unwrap();
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
            let mut conns = self.conns.write().unwrap();
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

        println!("TCP server listening on port {}", port);

        let conns = self.conns.clone();

        thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let conns = conns.clone();
                        let (tx, rx): (SyncSender<Vec<u8>>, Receiver<Vec<u8>>) =
                            sync_channel(buffer_size);

                        if Self::add_conn(&conns, tx).is_err() {
                            continue;
                        }

                        thread::spawn(move || {
                            handle_connection(stream, rx);
                        });
                    }
                    Err(e) => {
                        eprintln!("Error accepting connection: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    fn add_conn(
        conns: &Arc<RwLock<Vec<SyncSender<Vec<u8>>>>>,
        conn: SyncSender<Vec<u8>>,
    ) -> Result<(), &'static str> {
        let mut conns = conns.write().unwrap();
        conns.push(conn);
        Ok(())
    }
}

fn handle_connection(mut stream: TcpStream, rx: Receiver<Vec<u8>>) {
    for msg in rx {
        if let Err(e) = stream.write_all(&msg) {
            eprintln!("Error writing data: {}", e);
            break;
        }
    }
}
