//! FlatBuffer serialization module
use crate::flatbuffer::account_info_generated::account_info::{AccountInfo, AccountInfoArgs};
use crate::flatbuffer::consts::{
    BYTE_PREFIX_ACCOUNT, BYTE_PREFIX_BLOCK, BYTE_PREFIX_METADATA, BYTE_PREFIX_SLOT, BYTE_PREFIX_TX,
};
use crate::flatbuffer::extractors::{
    extract_inner_instructions, extract_loaded_addresses_string, extract_return_data,
    extract_rewards, extract_token_balances_ptr, extract_tx_meta,
};

use crate::flatbuffer::update_types::{AccountUpdate, BlockUpdate, TransactionUpdate};
use crate::{
    errors::GeyserError,
    flatbuffer::transaction_info_generated::transaction_info::{
        TransactionInfo, TransactionInfoArgs,
    },
};
use flatbuffers::FlatBufferBuilder;
use solana_geyser_plugin_interface::geyser_plugin_interface::SlotStatus;
pub use solana_program::hash::Hash;

use self::account_data_generated::account_data::{AccountData, AccountDataArgs};
use self::metadata_generated::metadata::{Metadata, MetadataArgs};
use self::{
    block_info_generated::block_info::{BlockInfo, BlockInfoArgs},
    slot_generated::slot::{Slot, SlotArgs, Status},
};

#[allow(dead_code, clippy::all)]
mod account_data_generated;
#[allow(dead_code, clippy::all)]
mod account_info_generated;
#[allow(dead_code, clippy::all)]
mod block_info_generated;
#[allow(dead_code, clippy::all)]
mod common_generated;
mod consts;
mod extractors;
#[allow(dead_code, clippy::all)]
mod metadata_generated;
#[allow(dead_code, clippy::all)]
mod slot_generated;
#[allow(dead_code, clippy::all)]
mod transaction_info_generated;
pub mod update_types;

/// Struct which implements FlatBuffer serialization for accounts, block metadata and transactions data
#[derive(Debug, Copy, Clone)]
pub struct FlatBufferSerialization {}

pub fn serialize_account(account: &AccountUpdate) -> Vec<u8> {
    let mut data_builder = FlatBufferBuilder::new();
    let data = Some(data_builder.create_vector(account.data.as_ref()));
    let account_data = AccountData::create(
        &mut data_builder,
        &AccountDataArgs {
            lamports: account.lamports,
            rent_epoch: account.rent_epoch,
            executable: account.executable,
            version: account.write_version,
            data,
        },
    );
    data_builder.finish(account_data, None);

    let mut builder = FlatBufferBuilder::new();
    let pubkey = Some(builder.create_string(account.key.to_string().as_ref()));
    let owner = Some(builder.create_string(account.owner.to_string().as_ref()));
    let account_data = Some(builder.create_vector(data_builder.finished_data()));

    let account_info = AccountInfo::create(
        &mut builder,
        &AccountInfoArgs {
            pubkey,
            owner,
            slot: account.slot,
            account_data,
        },
    );

    builder.finish(account_info, None);

    build_output(BYTE_PREFIX_ACCOUNT, builder.finished_data().to_vec())
}

pub fn serialize_slot(slot: u64, parent: Option<u64>, status: SlotStatus) -> Vec<u8> {
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
            parent,
        },
    );

    builder.finish(s, None);

    build_output(BYTE_PREFIX_SLOT, builder.finished_data().to_vec())
}

pub fn serialize_block(block: &BlockUpdate) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();

    let rewards = extract_rewards(&block.rewards.to_vec().into(), &mut builder);

    let blockhash = builder.create_string(block.blockhash);
    let parent_blockhash = if block.parent_blockhash.is_some() {
        Some(builder.create_string(block.parent_blockhash.unwrap()))
    } else {
        None
    };

    let b = BlockInfo::create(
        &mut builder,
        &BlockInfoArgs {
            slot: block.slot,
            blockhash: Some(blockhash),
            block_time: block.block_time.unwrap_or(0),
            block_height: block.block_height.unwrap_or(0),
            parent_slot: block.parent_slot,
            parent_blockhash,
            rewards,
            executed_transaction_count: block.executed_transaction_count,
        },
    );

    builder.finish(b, None);

    build_output(BYTE_PREFIX_BLOCK, builder.finished_data().to_vec())
}

pub fn serialize_transaction(transaction: &TransactionUpdate) -> Result<Vec<u8>, GeyserError> {
    let mut builder = FlatBufferBuilder::new();

    let transaction_meta = extract_tx_meta(&transaction.transaction_meta, &mut builder);

    let transaction_serialized =
        bincode::serialize(&transaction.transaction.to_versioned_transaction())
            .map_err(|_| GeyserError::TxSerializeError)?;
    let transaction_serialized = Some(builder.create_vector(&transaction_serialized));

    let pre_token_balances_ptr = extract_token_balances_ptr(
        &transaction.transaction_meta.pre_token_balances,
        &mut builder,
    );
    let post_token_balances_ptr = extract_token_balances_ptr(
        &transaction.transaction_meta.post_token_balances,
        &mut builder,
    );

    let loaded_addresses_string =
        extract_loaded_addresses_string(transaction.transaction.message(), &mut builder);
    let signature_string = Some(builder.create_string(transaction.signature.to_string().as_str()));

    let memo = solana_transaction_status::extract_memos::extract_and_fmt_memos(
        transaction.transaction.message(),
    )
    .map(|m| builder.create_string(&m));

    let account_keys = transaction
        .transaction
        .message()
        .account_keys()
        .iter()
        .map(|key| builder.create_string(key.to_string().as_str()))
        .collect::<Vec<_>>();
    let account_keys_string = Some(builder.create_vector(account_keys.as_ref()));

    let return_data = extract_return_data(&transaction.transaction_meta.return_data, &mut builder);

    let inner_instructions = extract_inner_instructions(
        &transaction.transaction_meta.inner_instructions,
        &mut builder,
    );

    let transaction_info = TransactionInfo::create(
        &mut builder,
        &TransactionInfoArgs {
            signature_string,
            is_vote: transaction.is_vote,
            slot: transaction.slot,
            transaction: transaction_serialized,
            transaction_meta,
            loaded_addresses_string,
            pre_token_balances_ptr,
            account_keys_string,
            memo,
            return_data,
            compute_units_consumed: transaction.transaction_meta.compute_units_consumed,
            index: transaction.index.map(|index| index as u64),
            signature: None,
            account_keys: None,
            loaded_addresses: None,
            post_token_balances_ptr,
            inner_instructions,
        },
    );
    builder.finish(transaction_info, None);

    Ok(build_output(
        BYTE_PREFIX_TX,
        builder.finished_data().to_vec(),
    ))
}

pub fn serialize_metadata(send_errors: u64) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();

    let obj = Metadata::create(&mut builder, &MetadataArgs { send_errors });
    builder.finish(obj, None);

    build_output(BYTE_PREFIX_METADATA, builder.finished_data().to_vec())
}

fn build_output(prefix: u8, data: Vec<u8>) -> Vec<u8> {
    let mut output = vec![prefix];
    output.extend(data);

    output
}
