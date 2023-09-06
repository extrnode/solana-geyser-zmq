use log::{debug, error, info};
use std::io::{self, Read};
use std::net::{SocketAddr, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use crate::sender::HEADER_BYTE_SIZE;

#[allow(unused)]
pub struct TcpReceiver {
    callback: Box<dyn Fn(Vec<u8>)>,
    connect_timeout: Duration,
    reconnect_interval: Duration,
}

#[allow(unused)]
impl TcpReceiver {
    pub fn new(
        callback: Box<dyn Fn(Vec<u8>) + Send>,
        connect_timeout: Duration,
        reconnect_interval: Duration,
    ) -> TcpReceiver {
        TcpReceiver {
            callback,
            connect_timeout,
            reconnect_interval,
        }
    }

    pub fn connect(&self, addr: SocketAddr) -> io::Result<()> {
        loop {
            info!("Receiver Connect {:?}", addr);

            if let Err(e) = self.connect_and_read(addr) {
                error!("receiver: read error: {:?}", e);
            }

            thread::sleep(self.reconnect_interval);
        }
    }

    fn connect_and_read(&self, addr: SocketAddr) -> io::Result<()> {
        let mut stream = TcpStream::connect_timeout(&addr, self.connect_timeout)?;

        loop {
            let (bytes_read, duration, num_elements) = self.read_response(&mut stream)?;
            debug!(
                "TCP Socket: Received {} elements, {} in {:?}",
                num_elements, bytes_read, duration
            );
        }
    }

    fn read_response(&self, stream: &mut TcpStream) -> io::Result<(usize, Duration, u32)> {
        let mut header = [0; HEADER_BYTE_SIZE];
        stream.read_exact(&mut header)?;

        let mut body = vec![0; u32::from_le_bytes(header) as usize];
        let now = Instant::now();
        stream.read_exact(&mut body)?;

        let duration = now.elapsed();
        let bytes_read = header.len() + body.len();
        let mut num_elements = 0;

        let mut i = 0;
        while i < body.len() {
            let mut end = i + HEADER_BYTE_SIZE;
            let size_bytes = body[i..end].try_into().map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to convert size: {}", e),
                )
            })?;
            let size = u32::from_le_bytes(size_bytes) as usize;
            i = end;

            end = i + size;
            (self.callback)(body[i..end].to_vec());
            i = end;

            num_elements += 1;
        }

        Ok((bytes_read, duration, num_elements))
    }
}
