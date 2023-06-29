use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub port: u16,

    pub send_transactions: bool,
    pub send_accounts: bool,
    pub send_blocks: bool,

    pub skip_vote_txs: bool,
}

impl Config {
    pub fn read(path: &str) -> Result<Self, std::io::Error> {
        let data = std::fs::read_to_string(path)?;
        let c: Config = serde_json::from_str(data.as_str())?;

        Ok(c)
    }
}
