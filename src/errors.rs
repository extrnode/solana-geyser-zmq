use thiserror::Error;

#[derive(Error, Debug)]
pub enum GeyserError<'a> {
    #[error("zmq send error")]
    ZmqSend,

    #[error("custom error: {0}")]
    CustomError(&'a str),

    #[error("sqlite error: {0}")]
    SqliteError(sqlite::Error),
}
