use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub tcp_port: u16,
    pub tcp_buffer_size: usize,
    pub tcp_batch_max_bytes: usize,

    // if set to true, messages will not be dropped when tcp_buffer_size is full
    // instead the application will reattempt to send until the buffer has enough space
    // NOTE: not to be used in production, but can be helpful for snapshot publishing
    pub tcp_strict_delivery: Option<bool>,

    // if set to positive number, the execution will hang until enough subscribers are available
    // NOTE: not to be used in production, but can be helpful for snapshot publishing
    pub tcp_min_subscribers: Option<usize>,

    pub send_transactions: bool,
    pub send_accounts: bool,
    pub send_blocks: bool,

    pub skip_vote_txs: bool,
    pub skip_deploy_txs: bool,
}

impl Config {
    pub fn read(path: &str) -> Result<Self, std::io::Error> {
        let data = std::fs::read_to_string(path)?;
        let c: Config = serde_json::from_str(data.as_str())?;

        Ok(c)
    }
}
