use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub port: usize,
}

impl Config {
    pub fn read(path: &str) -> Result<Self, std::io::Error> {
        let data = std::fs::read_to_string(path)?;
        let c: Config = serde_json::from_str(data.as_str())?;

        Ok(c)
    }
}
