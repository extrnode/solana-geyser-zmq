use thiserror::Error;

#[derive(Error, Debug)]
pub enum GeyserError {
    #[error("zmq send error")]
    TcpSend(usize),

    #[error("tx serialization error")]
    TxSerializeError,
}
