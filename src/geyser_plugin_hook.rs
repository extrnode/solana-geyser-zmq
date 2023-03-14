use crate::{
    config::Config,
    flatbuffer::{self, AccountUpdate},
};
use log::info;
use solana_geyser_plugin_interface::geyser_plugin_interface::*;
use solana_program::pubkey::Pubkey;
use std::sync::{Arc, Mutex};

use std::fmt::{Debug, Formatter};
/// This is the main object returned bu our dynamic library in entrypoint.rs
#[derive(Default)]
pub struct GeyserPluginHook {
    socket: Option<Arc<Mutex<zmq::Socket>>>,
}

impl GeyserPluginHook {
    fn send<'a>(&mut self, data: Vec<u8>) {
        self.socket
            .clone()
            .unwrap()
            .lock()
            .unwrap()
            .send(data, 0)
            .unwrap();
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

        solana_logger::setup_with_default("info");

        let ctx = zmq::Context::new();
        let socket = ctx.socket(zmq::PUSH).unwrap();

        let sndhwm = 1_000_000_000;
        socket.set_sndhwm(sndhwm).unwrap();
        socket
            .bind(format!("tcp://*:{}", cfg.port).as_str())
            .unwrap();

        info!("[on_load] - socket created");

        self.socket = Some(Arc::new(Mutex::new(socket)));

        Ok(())
    }

    /// Lifecycle: the plugin will be unloaded by the plugin manager
    /// Note: Do any cleanup necessary.
    fn on_unload(&mut self) {}

    /// Lifecycle: called when all accounts have been notified when the validator
    /// restores the AccountsDb from snapshots at startup.
    fn notify_end_of_startup(&mut self) -> Result<()> {
        Ok(())
    }

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

        match account {
            ReplicaAccountInfoVersions::V0_0_1(acc) => {
                let key = Pubkey::new_from_array(acc.pubkey.try_into().map_err(
                    |_| -> GeyserPluginError {
                        GeyserPluginError::AccountsUpdateError {
                            msg: "cannot decode pubkey".to_string(),
                        }
                    },
                )?);

                let owner = Pubkey::new_from_array(acc.owner.try_into().map_err(
                    |_| -> GeyserPluginError {
                        GeyserPluginError::AccountsUpdateError {
                            msg: "cannot decode owner".to_string(),
                        }
                    },
                )?);

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

                self.send(data);

                Ok(())
            }
        }
    }

    /// Event: a slot status is updated.
    fn update_slot_status(
        &mut self,
        slot: u64,
        _parent: Option<u64>,
        status: SlotStatus,
    ) -> Result<()> {
        let data = flatbuffer::serialize_slot(slot, status);
        self.send(data);

        Ok(())
    }

    /// Event: a transaction is updated at a slot.
    #[allow(unused_variables)]
    fn notify_transaction(
        &mut self,
        transaction: ReplicaTransactionInfoVersions,
        slot: u64,
    ) -> Result<()> {
        Ok(())
    }

    fn notify_block_metadata(&mut self, _blockinfo: ReplicaBlockInfoVersions) -> Result<()> {
        Ok(())
    }

    fn account_data_notifications_enabled(&self) -> bool {
        true
    }

    fn transaction_notifications_enabled(&self) -> bool {
        false
    }
}

/// Also required by GeyserPlugin trait
impl Debug for GeyserPluginHook {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "GeyserPluginHook")
    }
}
