use thiserror::Error;

#[derive(Error, Debug)]
pub enum GeyserError {
    #[error("zmq send error")]
    ZmqSend,

    #[error("tx serialization error")]
    TxSerializeError,
}
