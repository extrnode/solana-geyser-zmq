use crate::{
    config::Config,
    errors::GeyserError,
    flatbuffer::{self, AccountUpdate, TransactionUpdate},
    metrics::Metrics,
    sender::TcpSender,
};
use log::info;
use solana_geyser_plugin_interface::geyser_plugin_interface::*;
use solana_program::pubkey::Pubkey;
use std::{
    fmt::{Debug, Formatter},
    sync::atomic::Ordering,
    time::Duration,
};
use std::{sync::Arc, thread};

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

        let socket = TcpSender::default();
        socket.bind(cfg.port, 1_000_000).unwrap();

        info!("[on_load] - socket created");

        self.0 = Some(Arc::new(Inner {
            socket,
            metrics: metrics.clone(),
            config: cfg,
        }));

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
            |inner| {
                let account_update = match account {
                    ReplicaAccountInfoVersions::V0_0_1(acc) => {
                        let key = Pubkey::new_from_array(acc.pubkey.try_into()?);
                        let owner = Pubkey::new_from_array(acc.owner.try_into()?);

                        AccountUpdate {
                            key,
                            lamports: acc.lamports,
                            owner,
                            executable: acc.executable,
                            rent_epoch: acc.rent_epoch,
                            data: acc.data.to_vec(),
                            write_version: acc.write_version,
                            slot,
                            is_startup,
                        }
                    }
                    ReplicaAccountInfoVersions::V0_0_2(acc) => {
                        let key = Pubkey::new_from_array(acc.pubkey.try_into()?);
                        let owner = Pubkey::new_from_array(acc.owner.try_into()?);

                        AccountUpdate {
                            key,
                            lamports: acc.lamports,
                            owner,
                            executable: acc.executable,
                            rent_epoch: acc.rent_epoch,
                            data: acc.data.to_vec(),
                            write_version: acc.write_version,
                            slot,
                            is_startup,
                        }
                    }
                };
                let data = flatbuffer::serialize_account(&account_update);
                inner.socket.publish(data)?;

                Ok(())
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
        parent: Option<u64>,
        status: SlotStatus,
    ) -> Result<()> {
        self.with_inner(
            || GeyserPluginError::SlotStatusUpdateError { msg: UNINIT.into() },
            |inner| {
                let data = flatbuffer::serialize_slot(slot, parent, status);
                inner.socket.publish(data)?;

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
                let tx_update = TransactionUpdate::from_transaction(transaction, slot);
                if tx_update.is_vote && inner.config.skip_vote_txs {
                    return Ok(());
                }

                let data = flatbuffer::serialize_transaction(&tx_update)?;
                inner.socket.publish(data)?;

                Ok(())
            },
        )
    }

    fn notify_block_metadata(&mut self, blockinfo: ReplicaBlockInfoVersions) -> Result<()> {
        self.with_inner(
            || GeyserPluginError::SlotStatusUpdateError { msg: UNINIT.into() },
            |inner| {
                if !inner.config.send_blocks {
                    return Ok(());
                }

                match blockinfo {
                    ReplicaBlockInfoVersions::V0_0_1(block) => {
                        let data = flatbuffer::serialize_block(block);
                        inner.socket.publish(data)?;
                    }
                };

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

impl TransactionUpdate {
    fn from_transaction(tx: ReplicaTransactionInfoVersions, slot: u64) -> Self {
        match tx {
            ReplicaTransactionInfoVersions::V0_0_1(tx) => TransactionUpdate {
                signature: *tx.signature,
                is_vote: tx.is_vote,
                slot,
                transaction: tx.transaction.clone(),
                transaction_meta: tx.transaction_status_meta.clone(),
                index: None,
            },
            ReplicaTransactionInfoVersions::V0_0_2(tx) => TransactionUpdate {
                signature: *tx.signature,
                is_vote: tx.is_vote,
                slot,
                transaction: tx.transaction.clone(),
                transaction_meta: tx.transaction_status_meta.clone(),
                index: Some(tx.index),
            },
        }
    }
}
