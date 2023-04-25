use log::error;
use serde::Deserialize;
use solana_program::pubkey::Pubkey;
use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

#[derive(Deserialize)]
struct GeyserFilters {
    pub program_ids: Vec<String>,
}

pub fn load_tx_filters(filters: Arc<RwLock<HashMap<Pubkey, bool>>>, url: String) {
    let a = |u: String| -> std::result::Result<_, Box<dyn std::error::Error>> {
        let body = reqwest::blocking::get(u)?.text()?;
        let gf: GeyserFilters = serde_json::from_str(&body)?;

        let mut filters_write = filters.write()?;
        filters_write.clear();

        for pid in gf.program_ids {
            filters_write.insert(Pubkey::from_str(pid.as_str())?, true);
        }

        Ok(())
    };

    loop {
        match a(url.clone()) {
            Ok(_) => {}
            Err(e) => {
                error!("load_tx_filters = {:?}", e);
            }
        }

        thread::sleep(Duration::from_secs(10));
    }
}
