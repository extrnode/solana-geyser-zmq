use solana_program::{message::SanitizedMessage, pubkey::Pubkey};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    vec,
};

use crate::{db::DB, errors::GeyserError};

pub struct GeyserFilters {
    filters: RwLock<HashMap<Pubkey, bool>>,
    skip_votes: bool,
}

impl GeyserFilters {
    pub fn new_arc(db: &DB, skip_votes: bool) -> Arc<Self> {
        let filters = Arc::new(Self {
            filters: RwLock::new(HashMap::new()),
            skip_votes,
        });

        let existing_filters = db.get_filters();
        if existing_filters.is_ok() {
            filters.update_filters(existing_filters.unwrap()).unwrap();
        }

        filters
    }

    pub fn update_filters(&self, program_ids: Vec<Pubkey>) -> Result<(), GeyserError> {
        let mut f = self
            .filters
            .write()
            .map_err(|_| GeyserError::CustomError("cannot obtain lock"))?;

        f.clear();

        for pid in program_ids {
            f.insert(pid, true);
        }

        Ok(())
    }

    pub fn get_filters(&self) -> Vec<Pubkey> {
        match self.filters.read() {
            Ok(filters) => filters.iter().map(|(k, _)| k.clone()).collect(),
            Err(_) => {
                vec![]
            }
        }
    }

    pub fn should_send(&self, msg: &SanitizedMessage, is_vote: bool) -> bool {
        if is_vote {
            return !self.skip_votes;
        }

        match self.filters.read() {
            Ok(filters) => msg
                .account_keys()
                .iter()
                .find(|&&x| filters.contains_key(&x))
                .is_some(),
            Err(e) => {
                log::error!("Error reading account filters: {}", e);
                // if there's an issue with filters mutex, just send tx, in order not to loose anything
                true
            }
        }
    }
}
