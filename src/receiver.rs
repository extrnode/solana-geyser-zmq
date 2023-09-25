use log::{debug, error, info};
use std::convert::TryInto;
use std::future::Future;
use std::io::{self, Read};
use std::net::SocketAddr;
use std::pin::Pin;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::time::sleep;

const HEADER_BYTE_SIZE: usize = 4;

pub type Callback = Box<dyn Fn(Vec<u8>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

pub struct TcpReceiver {
    callback: Callback,
    connect_timeout: Duration,
    reconnect_interval: Duration,
}

impl TcpReceiver {
    pub fn new(
        callback: Callback,
        connect_timeout: Duration,
        reconnect_interval: Duration,
    ) -> TcpReceiver {
        TcpReceiver {
            callback,
            connect_timeout,
            reconnect_interval,
        }
    }

    pub async fn connect(&self, addr: SocketAddr) -> io::Result<()> {
        loop {
            info!("Receiver Connect {:?}", addr);

            if let Err(e) = self.connect_and_read(addr).await {
                error!("receiver: read error: {:?}", e);
            }

            sleep(self.reconnect_interval).await;
        }
    }

    async fn connect_and_read(&self, addr: SocketAddr) -> io::Result<()> {
        // TODO: there's no connect with timeout in tokio's TcpStream
        // but we can create std::net::TcpStream and convert it to tokio's one
        // like here https://docs.rs/tokio/latest/tokio/net/struct.TcpStream.html#method.from_std
        //
        // Example:
        // let std_stream = std::net::TcpStream::connect_timeout(&addr, self.connect_timeout)?;
        // std_stream.set_nonblocking(true)?;
        // let stream = TcpStream::from_std(std_stream)?;
        //
        // Another approach is to use tokio::time::timeout like here
        // https://stackoverflow.com/a/63465533
        //
        // I didn't test any of those approaches

        let stream = TcpStream::connect(&addr).await?;
        let mut stream = tokio::io::BufReader::new(stream);

        loop {
            let (bytes_read, duration, num_elements) = self.read_response(&mut stream).await?;
            debug!(
                "TCP Socket: Received {} elements, {} in {:?}",
                num_elements, bytes_read, duration
            );
        }
    }

    async fn read_response(
        &self,
        stream: &mut tokio::io::BufReader<TcpStream>,
    ) -> io::Result<(usize, Duration, u32)> {
        let mut header = [0; HEADER_BYTE_SIZE];
        stream.read_exact(&mut header).await?;

        let mut body = vec![0; u32::from_le_bytes(header) as usize];
        let now = Instant::now();
        stream.read_exact(&mut body).await?;

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
            (self.callback)(body[i..end].to_vec()).await;
            i = end;

            num_elements += 1;
        }

        Ok((bytes_read, duration, num_elements as u32))
    }
}
