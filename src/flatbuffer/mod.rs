//! FlatBuffer serialization module
use account_info_generated::account_info::{
    AccountInfo, AccountInfoArgs, Pubkey as AccountInfoPubkey, PubkeyArgs as AccountInfoPubkeyArgs,
};
use flatbuffers::FlatBufferBuilder;
use solana_geyser_plugin_interface::geyser_plugin_interface::{ReplicaTransactionInfo, SlotStatus};
pub use solana_program::pubkey::Pubkey;

use self::slot_generated::slot::{Slot, SlotArgs, Status};

#[allow(clippy::all)]
mod account_info_generated;
#[allow(clippy::all)]
mod slot_generated;

/// Struct which implements FlatBuffer serialization for accounts, block metadata and transactions data
#[derive(Debug, Copy, Clone)]
pub struct FlatBufferSerialization {}

const BYTE_PREFIX_ACCOUNT: u8 = 0;
const BYTE_PREFIX_SLOT: u8 = 1;
const BYTE_PREFIX_TX: u8 = 2;

pub struct AccountUpdate {
    /// The account's public key
    pub key: Pubkey,
    /// The lamport balance of the account
    pub lamports: u64,
    /// The Solana program controlling this account
    pub owner: Pubkey,
    /// True if the account's data is an executable smart contract
    pub executable: bool,
    /// The next epoch for which this account will owe rent
    pub rent_epoch: u64,
    /// The binary data stored on this account
    pub data: Vec<u8>,
    /// Monotonic-increasing counter for sequencing on-chain writes
    pub write_version: u64,
    /// The slot in which this account was updated
    pub slot: u64,
    /// True if this update was triggered by a validator startup
    pub is_startup: bool,
}

pub fn serialize_account(account: &AccountUpdate) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();

    let pubkey_vec = builder.create_vector(account.key.as_ref());
    let owner_vec = builder.create_vector(account.owner.as_ref());

    let pubkey = AccountInfoPubkey::create(
        &mut builder,
        &AccountInfoPubkeyArgs {
            key: Some(pubkey_vec),
        },
    );

    let owner = AccountInfoPubkey::create(
        &mut builder,
        &AccountInfoPubkeyArgs {
            key: Some(owner_vec),
        },
    );

    let data = builder.create_vector(account.data.as_ref());

    let account_info = AccountInfo::create(
        &mut builder,
        &AccountInfoArgs {
            pubkey: Some(pubkey),
            lamports: account.lamports,
            owner: Some(owner),
            executable: account.executable,
            rent_epoch: account.rent_epoch,
            data: Some(data),
            write_version: account.write_version,
            slot: account.slot,
            is_startup: account.is_startup,
        },
    );

    builder.finish(account_info, None);

    let mut output = vec![BYTE_PREFIX_ACCOUNT];
    output.extend(builder.finished_data().to_vec());

    output
}

pub fn serialize_slot<'a>(slot: u64, status: SlotStatus) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();

    let s = Slot::create(
        &mut builder,
        &SlotArgs {
            slot,
            status: match status {
                SlotStatus::Processed => Status::Processed,
                SlotStatus::Rooted => Status::Rooted,
                SlotStatus::Confirmed => Status::Confirmed,
            },
        },
    );

    builder.finish(s, None);

    let mut output = vec![BYTE_PREFIX_SLOT];
    output.extend(builder.finished_data().to_vec());

    output
}

pub fn serialize_tx<'a>(tx: &'a ReplicaTransactionInfo, slot: u64) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();

    builder.finish(s, None);

    let mut output = vec![BYTE_PREFIX_TX];
    output.extend(builder.finished_data().to_vec());

    output
}
