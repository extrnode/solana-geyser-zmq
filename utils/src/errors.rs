use thiserror::Error;

#[derive(Error, Debug)]
pub enum GeyserError {
    #[error("tcp send error")]
    TcpSend(u64),

    #[error("tcp send error due to subscriber disconnect")]
    TcpDisconnects(u64),

    #[error("cannot acquire sender lock")]
    SenderLockError,

    #[error("cannot acquire lock to add new conn")]
    ConnLockError,

    #[error("tx serialization error")]
    TxSerializeError,
}
