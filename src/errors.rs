use thiserror::Error;

#[derive(Error, Debug)]
pub enum GeyserError {
    #[error("tcp send error")]
    TcpSend(usize),

    #[error("cannot acquire sender lock")]
    SenderLockError,

    #[error("cannot acquire lock to add new conn")]
    ConnLockError,

    #[error("tx serialization error")]
    TxSerializeError,
}
