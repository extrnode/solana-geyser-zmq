use std::sync::{Arc, Mutex};

use solana_geyser_plugin_interface::geyser_plugin_interface::ReplicaAccountInfoV2;

use crate::flatbuffer;

use {
    log::*,
    solana_geyser_plugin_interface::geyser_plugin_interface::{
        GeyserPlugin, GeyserPluginError, ReplicaAccountInfoVersions, ReplicaBlockInfoVersions,
        ReplicaTransactionInfoVersions, SlotStatus,
    },
    std::fmt::{Debug, Formatter},
};

/// This is the main object returned bu our dynamic library in entrypoint.rs
#[derive(Default)]
pub struct GeyserPluginHook {
    socket: Option<Arc<Mutex<zmq::Socket>>>,
    serializer: Option<flatbuffer::FlatBufferSerialization>,
}

impl GeyserPluginHook {
    fn send<'a>(&mut self, account: &'a ReplicaAccountInfoV2<'a>, slot: u64, is_startup: bool) {
        let data = self
            .serializer
            .unwrap()
            .serialize_account(account, slot, is_startup);

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
    fn on_load(
        &mut self,
        config_file: &str,
    ) -> solana_geyser_plugin_interface::geyser_plugin_interface::Result<()> {
        solana_logger::setup_with_default("info");
        info!("[on_load] - config_file: {:#?}", config_file);

        let ctx = zmq::Context::new();
        let socket = ctx.socket(zmq::PUSH).unwrap();

        let sndhwm = 100_000_000;
        socket.set_sndhwm(sndhwm).unwrap();
        socket.bind("tcp://*:5555").unwrap();

        info!("[on_load] - socket created");

        self.socket = Some(Arc::new(Mutex::new(socket)));
        self.serializer = Some(flatbuffer::FlatBufferSerialization {});

        Ok(())
    }

    /// Lifecycle: the plugin will be unloaded by the plugin manager
    /// Note: Do any cleanup necessary.
    fn on_unload(&mut self) {
        info!("[on_unload]");
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
    ) -> solana_geyser_plugin_interface::geyser_plugin_interface::Result<()> {
        match account {
            ReplicaAccountInfoVersions::V0_0_1(_) => {
                return Err(GeyserPluginError::AccountsUpdateError {
                    msg: "ReplicaAccountInfoVersions::V0_0_1 it not supported".to_string(),
                });
            }
            ReplicaAccountInfoVersions::V0_0_2(acc) => {
                let acc_string = format!(
                    "pubkey: {}, owner: {}",
                    bs58::encode(acc.pubkey).into_string(),
                    bs58::encode(acc.owner).into_string(),
                );
                info!(
                    "[update_account] - account: {:#?}, slot:{:#?}, is_startup:{:#?}",
                    acc_string, slot, is_startup
                );

                self.send(acc, slot, is_startup);
            }
        }
        Ok(())
    }

    /// Lifecycle: called when all accounts have been notified when the validator
    /// restores the AccountsDb from snapshots at startup.
    fn notify_end_of_startup(
        &mut self,
    ) -> solana_geyser_plugin_interface::geyser_plugin_interface::Result<()> {
        info!("[notify_end_of_startup]");
        Ok(())
    }

    /// Event: a slot status is updated.
    fn update_slot_status(
        &mut self,
        slot: u64,
        parent: Option<u64>,
        status: SlotStatus,
    ) -> solana_geyser_plugin_interface::geyser_plugin_interface::Result<()> {
        info!(
            "[update_slot_status], slot:{:#?}, parent:{:#?}, status:{:#?}",
            slot, parent, status
        );
        Ok(())
    }

    /// Event: a transaction is updated at a slot.
    #[allow(unused_variables)]
    fn notify_transaction(
        &mut self,
        transaction: ReplicaTransactionInfoVersions,
        slot: u64,
    ) -> solana_geyser_plugin_interface::geyser_plugin_interface::Result<()> {
        // match transaction {
        //     ReplicaTransactionInfoVersions::V0_0_1(transaction_info) => {
        //         // info!("[notify_transaction], transaction:{:#?}, slot:{:#?}", transaction_info.is_vote, slot);
        //     }
        // }
        Ok(())
    }

    fn notify_block_metadata(
        &mut self,
        blockinfo: ReplicaBlockInfoVersions,
    ) -> solana_geyser_plugin_interface::geyser_plugin_interface::Result<()> {
        match blockinfo {
            ReplicaBlockInfoVersions::V0_0_1(blockinfo) => {
                info!("[notify_block_metadata], block_info:{:#?}", blockinfo);
            }
        }
        Ok(())
    }

    fn account_data_notifications_enabled(&self) -> bool {
        info!("[account_data_notifications_enabled] - plugin interface is asking if data notifs should be enabled?");
        true
    }

    fn transaction_notifications_enabled(&self) -> bool {
        info!("[transaction_notifications_enabled] - plugin interface is asking if transactions notifs should be enabled?");
        true
    }
}

/// Also required by GeyserPlugin trait
impl Debug for GeyserPluginHook {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "GeyserPluginHook")
    }
}
