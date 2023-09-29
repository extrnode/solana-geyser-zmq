use crate::fb_serializers::update_types::{AccountUpdate, TransactionUpdate};
use crate::fb_serializers::{
    serialize_account, serialize_block, serialize_metadata, serialize_slot, serialize_transaction,
};
use crate::{config::Config, metrics::Metrics};
use log::info;
use solana_geyser_plugin_interface::geyser_plugin_interface::*;
use std::{
    fmt::{Debug, Formatter},
    sync::atomic::Ordering,
    time::Duration,
};
use std::{sync::Arc, thread};
use utils::{errors::GeyserError, sender::TcpSender};

const UNINIT: &str = "Geyser plugin not initialized yet!";

/// This is the main object returned bu our dynamic library in entrypoint.rs
#[derive(Default)]
pub struct GeyserPluginHook(Option<Arc<Inner>>);

pub struct Inner {
    socket: TcpSender,
    metrics: Arc<Metrics>,
    config: Config,
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
                            GeyserError::TcpSend(amount) => {
                                inner
                                    .metrics
                                    .send_errs
                                    .fetch_add(*amount, Ordering::Relaxed);
                            }
                            GeyserError::TcpDisconnects(amount) => {
                                inner
                                    .metrics
                                    .disconnect_errs
                                    .fetch_add(*amount, Ordering::Relaxed);
                            }
                            GeyserError::TxSerializeError => {
                                inner.metrics.serialize_errs.fetch_add(1, Ordering::Relaxed);
                            }
                            GeyserError::SenderLockError => {
                                inner
                                    .metrics
                                    .sender_lock_errs
                                    .fetch_add(1, Ordering::Relaxed);
                            }
                            GeyserError::ConnLockError => {
                                inner.metrics.conn_lock_errs.fetch_add(1, Ordering::Relaxed);
                            }
                        }

                        Ok(())
                    } else {
                        inner.metrics.untyped_errs.fetch_add(1, Ordering::Relaxed);

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

        let metrics = Metrics::new_rc();

        let cfg = Config::read(config_file).unwrap();

        let socket = TcpSender::new(
            cfg.tcp_batch_max_bytes,
            cfg.tcp_strict_delivery.unwrap_or(false),
            cfg.tcp_min_subscribers.unwrap_or(0),
        );
        socket.bind(cfg.tcp_port, cfg.tcp_buffer_size).unwrap();

        info!("[on_load] - socket created");

        let plugin = Arc::new(Inner {
            socket,
            metrics: metrics.clone(),
            config: cfg,
        });

        self.0 = Some(plugin.clone());

        thread::spawn(move || loop {
            let data = serialize_metadata(metrics.send_errs.load(Ordering::Relaxed));
            if let Err(e) = plugin.socket.publish(data) {
                info!("{}", e);
            }

            info!("{}", metrics);
            thread::sleep(Duration::from_secs(10));
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
        &self,
        account: ReplicaAccountInfoVersions,
        slot: u64,
        is_startup: bool,
    ) -> Result<()> {
        if is_startup {
            return Ok(());
        }

        self.with_inner(
            || GeyserPluginError::AccountsUpdateError { msg: UNINIT.into() },
            |inner| {
                let data =
                    serialize_account(&AccountUpdate::from_account(account, slot, is_startup)?);
                inner.socket.publish(data)?;

                Ok(())
            },
        )
    }

    /// Lifecycle: called when all accounts have been notified when the validator
    /// restores the AccountsDb from snapshots at startup.
    fn notify_end_of_startup(&self) -> Result<()> {
        Ok(())
    }

    /// Event: a slot status is updated.
    fn update_slot_status(&self, slot: u64, parent: Option<u64>, status: SlotStatus) -> Result<()> {
        self.with_inner(
            || GeyserPluginError::SlotStatusUpdateError { msg: UNINIT.into() },
            |inner| {
                let data = serialize_slot(slot, parent, status);
                inner.socket.publish(data)?;

                Ok(())
            },
        )
    }

    /// Event: a transaction is updated at a slot.
    #[allow(unused_variables)]
    fn notify_transaction(
        &self,
        transaction: ReplicaTransactionInfoVersions,
        slot: u64,
    ) -> Result<()> {
        self.with_inner(
            || GeyserPluginError::TransactionUpdateError { msg: UNINIT.into() },
            |inner| {
                let tx_update = TransactionUpdate::from_transaction(transaction, slot);

                if inner.config.skip_vote_txs && tx_update.is_vote {
                    return Ok(());
                }

                if inner.config.skip_deploy_txs && tx_update.is_deploy_tx() {
                    return Ok(());
                }

                let data = serialize_transaction(&tx_update)?;
                inner.socket.publish(data)?;

                Ok(())
            },
        )
    }

    fn notify_block_metadata(&self, blockinfo: ReplicaBlockInfoVersions) -> Result<()> {
        self.with_inner(
            || GeyserPluginError::SlotStatusUpdateError { msg: UNINIT.into() },
            |inner| {
                if !inner.config.send_blocks {
                    return Ok(());
                }

                let data = serialize_block(&blockinfo.into());
                inner.socket.publish(data)?;

                Ok(())
            },
        )
    }

    fn account_data_notifications_enabled(&self) -> bool {
        if let Some(inner) = self.0.as_ref() {
            inner.config.send_accounts
        } else {
            false
        }
    }

    fn transaction_notifications_enabled(&self) -> bool {
        if let Some(inner) = self.0.as_ref() {
            inner.config.send_transactions
        } else {
            false
        }
    }
}

/// Also required by GeyserPlugin trait
impl Debug for GeyserPluginHook {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "GeyserPluginHook")
    }
}
