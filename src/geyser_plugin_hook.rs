use crate::{
    api::Api,
    config::Config,
    db::DB,
    filters::GeyserFilters,
    flatbuffer::{self, AccountUpdate, TransactionUpdate},
    metrics::Metrics,
};
use log::{error, info};
use solana_geyser_plugin_interface::geyser_plugin_interface::*;
use solana_program::pubkey::Pubkey;
use std::{
    fmt::{Debug, Formatter},
    sync::atomic::Ordering,
    time::Duration,
};
use std::{
    sync::{Arc, Mutex},
    thread,
};
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
const UNINIT: &str = "ZMQ plugin not initialized yet!";

/// This is the main object returned bu our dynamic library in entrypoint.rs
#[derive(Default)]
pub struct GeyserPluginHook(Option<Arc<Inner>>);

pub struct Inner {
    socket: Mutex<zmq::Socket>,
    zmq_flag: i32,
    metrics: Arc<Metrics>,
    filters: Arc<GeyserFilters>,
}

impl GeyserPluginHook {
    #[inline]
    fn with_inner(
        &self,
        uninit: impl FnOnce() -> GeyserPluginError,
        f: impl FnOnce(&Arc<Inner>) -> anyhow::Result<()>,
    ) -> Result<()> {
        match self.0 {
            Some(ref inner) => match f(inner) {
                Ok(_) => Ok(()),
                Err(e) => {
                    if let Some(e) = e.downcast_ref::<GeyserError>() {
                        // in case of zmq error do not fill the log, just inc the err counter
                        match e {
                            GeyserError::ZmqSend => {
                                inner.metrics.send_errs.fetch_add(1, Ordering::Relaxed);
                            }
                            GeyserError::CustomError(_) => todo!(),
                            GeyserError::SqliteError(_) => todo!(),
                        }

                        Ok(())
                    } else {
                        inner.metrics.errs.fetch_add(1, Ordering::Relaxed);

                        Err(GeyserPluginError::Custom(e.into()))
                    }
                }
            },
            None => Err(uninit()),
        }
    }
}

/// Implementation of GeyserPlugin trait/interface
/// https://docs.rs/solana-geyser-plugin-interface/latest/solana_geyser_plugin_interface/geyser_plugin_interface/trait.GeyserPlugin.html
impl GeyserPlugin for GeyserPluginHook {
    fn name(&self) -> &'static str {
        "GeyserPluginHook"
    }

    /// Lifecycle: the plugin has been loaded by the system
    /// used for doing whatever initialization is required by the plugin.
    /// The _config_file contains the name of the
    /// of the config file. The config must be in JSON format and
    /// include a field "libpath" indicating the full path
    /// name of the shared library implementing this interface.
    fn on_load(&mut self, config_file: &str) -> Result<()> {
        solana_logger::setup_with_default("info");

        let cfg = Config::read(config_file).unwrap();
        let metrics = Metrics::new_rc();
        let db = DB::new(cfg.sqlite_filepath);

        let ctx = zmq::Context::new();
        let socket = ctx.socket(zmq::PUSH).unwrap();

        let sndhwm = 1_000_000_000;
        socket.set_sndhwm(sndhwm).unwrap();
        socket
            .bind(format!("tcp://*:{}", cfg.zmq_port).as_str())
            .unwrap();

        info!("[on_load] - socket created");

        let filters = Arc::new(GeyserFilters::new());
        let existing_filters = db.get_filters();
        if existing_filters.is_ok() {
            filters.update_filters(existing_filters.unwrap()).unwrap();
        }

        self.0 = Some(Arc::new(Inner {
            socket: Mutex::new(socket),
            zmq_flag: if cfg.zmq_no_wait { zmq::DONTWAIT } else { 0 },
            metrics: metrics.clone(),
            filters: Arc::clone(&filters),
        }));

        if let Some(http_port) = cfg.http_port {
            let api = Api::new(http_port, filters);
            thread::spawn(move || api.start(db));
        }

        thread::spawn(move || {
            metrics.spin(Duration::from_secs(10));
        });

        Ok(())
    }

    /// Lifecycle: the plugin will be unloaded by the plugin manager
    /// Note: Do any cleanup necessary.
    fn on_unload(&mut self) {}

    /// Event: an account has been updated at slot
    /// - When `is_startup` is true, it indicates the account is loaded from
    /// snapshots when the validator starts up.
    /// - When `is_startup` is false, the account is updated during transaction processing.
    /// Note: The account is versioned, so you can decide how to handle the different
    /// implementations.
    fn update_account(
        &mut self,
        account: ReplicaAccountInfoVersions,
        slot: u64,
        is_startup: bool,
    ) -> Result<()> {
        if is_startup {
            return Ok(());
        }

        self.with_inner(
            || GeyserPluginError::AccountsUpdateError { msg: UNINIT.into() },
            |inner| match account {
                ReplicaAccountInfoVersions::V0_0_1(acc) => {
                    let key = Pubkey::new_from_array(acc.pubkey.try_into()?);
                    let owner = Pubkey::new_from_array(acc.owner.try_into()?);

                    let data = flatbuffer::serialize_account(&AccountUpdate {
                        key,
                        lamports: acc.lamports,
                        owner,
                        executable: acc.executable,
                        rent_epoch: acc.rent_epoch,
                        data: acc.data.to_vec(),
                        write_version: acc.write_version,
                        slot,
                        is_startup,
                    });

                    inner
                        .socket
                        .lock()
                        .unwrap()
                        .send(data, inner.zmq_flag)
                        .map_err(|_| GeyserError::ZmqSend)?;

                    Ok(())
                }
            },
        )
    }

    /// Lifecycle: called when all accounts have been notified when the validator
    /// restores the AccountsDb from snapshots at startup.
    fn notify_end_of_startup(&mut self) -> Result<()> {
        Ok(())
    }

    /// Event: a slot status is updated.
    fn update_slot_status(
        &mut self,
        slot: u64,
        _parent: Option<u64>,
        status: SlotStatus,
    ) -> Result<()> {
        self.with_inner(
            || GeyserPluginError::SlotStatusUpdateError { msg: UNINIT.into() },
            |inner| {
                let data = flatbuffer::serialize_slot(slot, status);
                inner
                    .socket
                    .lock()
                    .unwrap()
                    .send(data, inner.zmq_flag)
                    .map_err(|_| GeyserError::ZmqSend)?;

                Ok(())
            },
        )
    }

    /// Event: a transaction is updated at a slot.
    #[allow(unused_variables)]
    fn notify_transaction(
        &mut self,
        transaction: ReplicaTransactionInfoVersions,
        slot: u64,
    ) -> Result<()> {
        self.with_inner(
            || GeyserPluginError::TransactionUpdateError { msg: UNINIT.into() },
            |inner| {
                match transaction {
                    ReplicaTransactionInfoVersions::V0_0_1(tx) => {
                        if !tx.is_vote && inner.filters.should_send(tx.transaction.message()) {
                            let data = flatbuffer::serialize_transaction(&TransactionUpdate {
                                signature: *tx.signature,
                                is_vote: tx.is_vote,
                                slot,
                                transaction: tx.transaction.clone(),
                                transaction_meta: tx.transaction_status_meta.clone(),
                            });
                            inner
                                .socket
                                .lock()
                                .unwrap()
                                .send(data, inner.zmq_flag)
                                .map_err(|_| GeyserError::ZmqSend)?;
                        }
                    }
                };

                Ok(())
            },
        )
    }

    fn notify_block_metadata(&mut self, _blockinfo: ReplicaBlockInfoVersions) -> Result<()> {
        Ok(())
    }

    fn account_data_notifications_enabled(&self) -> bool {
        true
    }

    fn transaction_notifications_enabled(&self) -> bool {
        true
    }
}

/// Also required by GeyserPlugin trait
impl Debug for GeyserPluginHook {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "GeyserPluginHook")
    }
}
