use crate::{
    config::Config,
    filters::load_tx_filters,
    flatbuffer::{self, AccountUpdate},
    metrics::Metrics,
};
use log::{error, info};
use solana_geyser_plugin_interface::geyser_plugin_interface::*;
use solana_program::pubkey::Pubkey;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::RwLock,
};
use std::{
    sync::{Arc, Mutex},
    thread,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GeyserError {
    #[error("zmq send error")]
    ZmqSend,
}
const UNINIT: &str = "ZMQ plugin not initialized yet!";

/// This is the main object returned bu our dynamic library in entrypoint.rs
#[derive(Default)]
pub struct GeyserPluginHook(Option<Arc<Inner>>);

pub struct Inner {
    socket: Mutex<zmq::Socket>,
    zmq_flag: i32,
    metrics: Arc<Metrics>,
    filters: Arc<RwLock<HashMap<Pubkey, bool>>>,
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
                    inner.metrics.errs.log(1);

                    if let Some(e) = e.downcast_ref::<GeyserError>() {
                        // in case of zmq error do not fill the log, just inc the err counter
                        match e {
                            GeyserError::ZmqSend => {
                                inner.metrics.send_errs.log(1);
                            }
                        }

                        Ok(())
                    } else {
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
        let cfg = Config::read(config_file).unwrap();

        let metrics = Metrics::new_rc();

        solana_logger::setup_with_default("info");

        let ctx = zmq::Context::new();
        let socket = ctx.socket(zmq::PUSH).unwrap();

        let sndhwm = 1_000_000_000;
        socket.set_sndhwm(sndhwm).unwrap();
        socket
            .bind(format!("tcp://*:{}", cfg.zmq_port).as_str())
            .unwrap();

        info!("[on_load] - socket created");

        let filters = Arc::new(RwLock::new(HashMap::new()));

        self.0 = Some(Arc::new(Inner {
            socket: Mutex::new(socket),
            zmq_flag: if cfg.zmq_no_wait { zmq::DONTWAIT } else { 0 },
            metrics,
            filters: Arc::clone(&filters),
        }));

        thread::spawn(|| load_tx_filters(filters, cfg.filters_url));

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
                let filters = inner.filters.read().unwrap();
                info!("filters {:?}", filters.keys());

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
