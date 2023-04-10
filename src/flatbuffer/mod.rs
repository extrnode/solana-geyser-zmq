//! FlatBuffer serialization module
use account_info_generated::account_info::{AccountInfo, AccountInfoArgs};
use common_generated::{
    Pubkey as FlatBufferPubkey, PubkeyArgs as FlatBufferPubkeyArgs,
    Signature as FlatBufferSignature, SignatureArgs as FlatBufferSignatureArgs,
};
use flatbuffers::{FlatBufferBuilder, WIPOffset};
use solana_geyser_plugin_interface::geyser_plugin_interface::SlotStatus;
pub use solana_program::hash::Hash;
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signature;

use crate::flatbuffer::transaction_info_generated::transaction_info::{
    CompiledInstruction, CompiledInstructionArgs, InnerInstructions, InnerInstructionsArgs,
    LegacyMessage, LegacyMessageArgs, LoadedAddresses, LoadedAddressesArgs, LoadedMessageV0,
    LoadedMessageV0Args, MessageAddressTableLookup, MessageAddressTableLookupArgs, MessageHeader,
    MessageHeaderArgs, MessageV0, MessageV0Args, Reward, RewardArgs, RewardType, SanitizedMessage,
    SanitizedTransaction, SanitizedTransactionArgs, TransactionInfo, TransactionInfoArgs,
    TransactionStatusMeta, TransactionStatusMetaArgs, TransactionTokenBalance,
    TransactionTokenBalanceArgs, UiTokenAmount, UiTokenAmountArgs,
};

use self::slot_generated::slot::{Slot, SlotArgs, Status};

#[allow(dead_code)]
mod account_info_generated;
#[allow(dead_code)]
mod common_generated;
#[allow(dead_code)]
mod slot_generated;
#[allow(dead_code)]
mod transaction_info_generated;

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

    let pubkey = FlatBufferPubkey::create(
        &mut builder,
        &FlatBufferPubkeyArgs {
            key: Some(pubkey_vec),
        },
    );

    let owner = FlatBufferPubkey::create(
        &mut builder,
        &FlatBufferPubkeyArgs {
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

#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct TransactionUpdate {
    pub signature: Signature,
    pub is_vote: bool,
    pub slot: u64,
    pub transaction: solana_sdk::transaction::SanitizedTransaction,
    pub transaction_meta: solana_transaction_status::TransactionStatusMeta,
}

pub fn serialize_transaction(transaction: &TransactionUpdate) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();

    fn make_pubkey<'fbb>(
        builder: &mut FlatBufferBuilder<'fbb>,
        key: &solana_sdk::pubkey::Pubkey,
    ) -> WIPOffset<FlatBufferPubkey<'fbb>> {
        let pubkey_vec = builder.create_vector(key.as_ref());

        FlatBufferPubkey::create(
            builder,
            &FlatBufferPubkeyArgs {
                key: Some(pubkey_vec),
            },
        )
    }

    fn make_signature<'fbb>(
        builder: &mut FlatBufferBuilder<'fbb>,
        signature: &solana_sdk::signature::Signature,
    ) -> WIPOffset<FlatBufferSignature<'fbb>> {
        let signature_vec = builder.create_vector(signature.as_ref());

        FlatBufferSignature::create(
            builder,
            &FlatBufferSignatureArgs {
                key: Some(signature_vec),
            },
        )
    }

    let message = Some(match &transaction.transaction.message() {
        solana_sdk::message::SanitizedMessage::Legacy(legacy_message) => {
            let header = Some(MessageHeader::create(
                &mut builder,
                &MessageHeaderArgs {
                    num_required_signatures: legacy_message.header.num_required_signatures,
                    num_readonly_signed_accounts: legacy_message
                        .header
                        .num_readonly_signed_accounts,
                    num_readonly_unsigned_accounts: legacy_message
                        .header
                        .num_readonly_unsigned_accounts,
                },
            ));

            let mut compiled_instructions = Vec::with_capacity(legacy_message.instructions.len());

            for instruction in &legacy_message.instructions {
                let accounts = Some(builder.create_vector(instruction.accounts.as_ref()));
                let data = Some(builder.create_vector(instruction.data.as_ref()));
                compiled_instructions.push(CompiledInstruction::create(
                    &mut builder,
                    &CompiledInstructionArgs {
                        program_id_index: instruction.program_id_index,
                        accounts,
                        data,
                    },
                ))
            }

            let account_keys = legacy_message
                .account_keys
                .iter()
                .map(|key| make_pubkey(&mut builder, key))
                .collect::<Vec<_>>();
            let account_keys = Some(builder.create_vector(account_keys.as_ref()));
            let recent_blockhash =
                Some(builder.create_vector(legacy_message.recent_blockhash.as_ref()));
            let instructions = Some(builder.create_vector(compiled_instructions.as_ref()));
            LegacyMessage::create(
                &mut builder,
                &LegacyMessageArgs {
                    header,
                    account_keys,
                    recent_blockhash,
                    instructions,
                },
            )
            .as_union_value()
        }
        solana_sdk::message::SanitizedMessage::V0(loaded_message_v0) => {
            let mut address_table_lookups =
                Vec::with_capacity(loaded_message_v0.message.address_table_lookups.len());
            for message_address_table_lookup in &loaded_message_v0.message.address_table_lookups {
                let writable_indexes = Some(
                    builder.create_vector(message_address_table_lookup.writable_indexes.as_ref()),
                );
                let readonly_indexes = Some(
                    builder.create_vector(message_address_table_lookup.readonly_indexes.as_ref()),
                );
                let account_key = Some(make_pubkey(
                    &mut builder,
                    &message_address_table_lookup.account_key,
                ));
                address_table_lookups.push(MessageAddressTableLookup::create(
                    &mut builder,
                    &MessageAddressTableLookupArgs {
                        account_key,
                        writable_indexes,
                        readonly_indexes,
                    },
                ));
            }

            let mut compiled_instructions =
                Vec::with_capacity(loaded_message_v0.message.instructions.len());

            for instruction in &loaded_message_v0.message.instructions {
                let accounts = Some(builder.create_vector(instruction.accounts.as_ref()));
                let data = Some(builder.create_vector(instruction.data.as_ref()));
                compiled_instructions.push(CompiledInstruction::create(
                    &mut builder,
                    &CompiledInstructionArgs {
                        program_id_index: instruction.program_id_index,
                        accounts,
                        data,
                    },
                ))
            }

            let header = Some(MessageHeader::create(
                &mut builder,
                &MessageHeaderArgs {
                    num_required_signatures: loaded_message_v0
                        .message
                        .header
                        .num_required_signatures,
                    num_readonly_signed_accounts: loaded_message_v0
                        .message
                        .header
                        .num_readonly_signed_accounts,
                    num_readonly_unsigned_accounts: loaded_message_v0
                        .message
                        .header
                        .num_readonly_unsigned_accounts,
                },
            ));

            let instructions = Some(builder.create_vector(compiled_instructions.as_ref()));
            let account_keys = loaded_message_v0
                .message
                .account_keys
                .iter()
                .map(|key| make_pubkey(&mut builder, key))
                .collect::<Vec<_>>();
            let account_keys = Some(builder.create_vector(account_keys.as_ref()));
            let address_table_lookups = Some(builder.create_vector(address_table_lookups.as_ref()));
            let recent_blockhash =
                Some(builder.create_vector(loaded_message_v0.message.recent_blockhash.as_ref()));
            let message_v0 = MessageV0::create(
                &mut builder,
                &MessageV0Args {
                    header,
                    account_keys,
                    recent_blockhash,
                    instructions,
                    address_table_lookups,
                },
            );

            let writable = loaded_message_v0
                .loaded_addresses
                .writable
                .iter()
                .map(|key| make_pubkey(&mut builder, key))
                .collect::<Vec<_>>();
            let writable = Some(builder.create_vector(writable.as_ref()));

            let readonly = loaded_message_v0
                .loaded_addresses
                .readonly
                .iter()
                .map(|key| make_pubkey(&mut builder, key))
                .collect::<Vec<_>>();
            let readonly = Some(builder.create_vector(readonly.as_ref()));

            let loaded_addresses =
                LoadedAddresses::create(&mut builder, &LoadedAddressesArgs { writable, readonly });

            LoadedMessageV0::create(
                &mut builder,
                &LoadedMessageV0Args {
                    message: Some(message_v0),
                    loaded_addresses: Some(loaded_addresses),
                },
            )
            .as_union_value()
        }
    });

    let message_type = match transaction.transaction.message() {
        solana_sdk::message::SanitizedMessage::Legacy(_) => SanitizedMessage::Legacy,
        solana_sdk::message::SanitizedMessage::V0(_) => SanitizedMessage::V0,
    };

    let message_hash = Some(builder.create_vector(transaction.transaction.message_hash().as_ref()));
    let signatures = transaction
        .transaction
        .signatures()
        .iter()
        .map(|signature| make_signature(&mut builder, signature))
        .collect::<Vec<_>>();
    let signatures = Some(builder.create_vector(signatures.as_ref()));
    let sanitized_transaction = Some(SanitizedTransaction::create(
        &mut builder,
        &SanitizedTransactionArgs {
            message_type,
            message,
            message_hash,
            is_simple_vote_tx: transaction.transaction.is_simple_vote_transaction(),
            signatures,
        },
    ));

    let inner_instructions =
        if let Some(inner_instructions) = &transaction.transaction_meta.inner_instructions {
            let mut inner_instructions_vec = Vec::with_capacity(inner_instructions.len());

            for inner_instruction in inner_instructions {
                let mut compiled_instructions =
                    Vec::with_capacity(inner_instruction.instructions.len());

                for instruction in &inner_instruction.instructions {
                    let accounts = Some(builder.create_vector(instruction.accounts.as_ref()));
                    let data = Some(builder.create_vector(instruction.data.as_ref()));
                    compiled_instructions.push(CompiledInstruction::create(
                        &mut builder,
                        &CompiledInstructionArgs {
                            program_id_index: instruction.program_id_index,
                            accounts,
                            data,
                        },
                    ))
                }

                let instructions = Some(builder.create_vector(compiled_instructions.as_ref()));
                inner_instructions_vec.push(InnerInstructions::create(
                    &mut builder,
                    &InnerInstructionsArgs {
                        index: inner_instruction.index,
                        instructions,
                    },
                ));
            }

            Some(builder.create_vector(inner_instructions_vec.as_ref()))
        } else {
            None
        };

    let pre_token_balances = if let Some(pre_token_balances) =
        &transaction.transaction_meta.pre_token_balances
    {
        let mut pre_token_balances_vec = Vec::with_capacity(pre_token_balances.len());
        for transaction_token_balance in pre_token_balances {
            let amount =
                Some(builder.create_string(&transaction_token_balance.ui_token_amount.amount));
            let ui_amount_string = Some(
                builder.create_string(&transaction_token_balance.ui_token_amount.ui_amount_string),
            );
            let decimals = transaction_token_balance.ui_token_amount.decimals;
            let ui_amount = if transaction_token_balance
                .ui_token_amount
                .ui_amount
                .is_some()
            {
                transaction_token_balance.ui_token_amount.ui_amount.unwrap()
            } else {
                0.0
            };

            let ui_token_amount = Some(UiTokenAmount::create(
                &mut builder,
                &UiTokenAmountArgs {
                    ui_amount,
                    decimals,
                    amount,
                    ui_amount_string,
                },
            ));

            let mint = Some(builder.create_string(&transaction_token_balance.mint));
            let owner = Some(builder.create_string(&transaction_token_balance.owner));
            let program_id = Some(builder.create_string(&transaction_token_balance.program_id));

            pre_token_balances_vec.push(TransactionTokenBalance::create(
                &mut builder,
                &TransactionTokenBalanceArgs {
                    account_index: transaction_token_balance.account_index,
                    mint,
                    ui_token_amount,
                    owner,
                    program_id,
                },
            ));
        }
        Some(builder.create_vector(pre_token_balances_vec.as_ref()))
    } else {
        None
    };

    let post_token_balances = if let Some(post_token_balances) =
        &transaction.transaction_meta.post_token_balances
    {
        let mut post_token_balances_vec = Vec::with_capacity(post_token_balances.len());
        for transaction_token_balance in post_token_balances {
            let amount =
                Some(builder.create_string(&transaction_token_balance.ui_token_amount.amount));
            let ui_amount_string = Some(
                builder.create_string(&transaction_token_balance.ui_token_amount.ui_amount_string),
            );
            let decimals = transaction_token_balance.ui_token_amount.decimals;
            let ui_amount = if transaction_token_balance
                .ui_token_amount
                .ui_amount
                .is_some()
            {
                transaction_token_balance.ui_token_amount.ui_amount.unwrap()
            } else {
                0.0
            };

            let ui_token_amount = Some(UiTokenAmount::create(
                &mut builder,
                &UiTokenAmountArgs {
                    ui_amount,
                    decimals,
                    amount,
                    ui_amount_string,
                },
            ));

            let mint = Some(builder.create_string(&transaction_token_balance.mint));
            let owner = Some(builder.create_string(&transaction_token_balance.owner));
            let program_id = Some(builder.create_string(&transaction_token_balance.program_id));

            post_token_balances_vec.push(TransactionTokenBalance::create(
                &mut builder,
                &TransactionTokenBalanceArgs {
                    account_index: transaction_token_balance.account_index,
                    mint,
                    ui_token_amount,
                    owner,
                    program_id,
                },
            ));
        }
        Some(builder.create_vector(post_token_balances_vec.as_ref()))
    } else {
        None
    };

    let rewards = if let Some(rewards) = &transaction.transaction_meta.rewards {
        let mut rewards_vec = Vec::with_capacity(rewards.len());
        for reward in rewards {
            let pubkey = Some(builder.create_string(&reward.pubkey));
            let reward_type = if let Some(rwrd_type) = reward.reward_type {
                match rwrd_type {
                    solana_transaction_status::RewardType::Fee => RewardType::Fee,
                    solana_transaction_status::RewardType::Rent => RewardType::Rent,
                    solana_transaction_status::RewardType::Staking => RewardType::Staking,
                    solana_transaction_status::RewardType::Voting => RewardType::Voting,
                }
            } else {
                RewardType::None
            };
            let commission = if let Some(commission) = reward.commission {
                commission
            } else {
                0
            };

            rewards_vec.push(Reward::create(
                &mut builder,
                &RewardArgs {
                    pubkey,
                    lamports: reward.lamports,
                    post_balance: reward.post_balance,
                    reward_type,
                    commission,
                },
            ));
        }

        Some(builder.create_vector(rewards_vec.as_ref()))
    } else {
        None
    };

    let pre_balances =
        Some(builder.create_vector(transaction.transaction_meta.pre_balances.as_ref()));
    let post_balances =
        Some(builder.create_vector(transaction.transaction_meta.post_balances.as_ref()));
    let log_messages = if let Some(logs) = &transaction.transaction_meta.log_messages {
        let log_messages = logs
            .iter()
            .map(|log| builder.create_string(log))
            .collect::<Vec<_>>();
        Some(builder.create_vector(log_messages.as_ref()))
    } else {
        None
    };

    let transaction_meta = Some(TransactionStatusMeta::create(
        &mut builder,
        &TransactionStatusMetaArgs {
            status: transaction.transaction_meta.status.is_ok(),
            fee: transaction.transaction_meta.fee,
            pre_balances,
            post_balances,
            inner_instructions,
            log_messages,
            pre_token_balances,
            post_token_balances,
            rewards,
        },
    ));

    let signature = Some(make_signature(&mut builder, &transaction.signature));
    let transaction_info = TransactionInfo::create(
        &mut builder,
        &TransactionInfoArgs {
            signature,
            is_vote: transaction.is_vote,
            slot: transaction.slot,
            transaction: sanitized_transaction,
            transaction_meta,
        },
    );
    builder.finish(transaction_info, None);

    let mut output = vec![BYTE_PREFIX_TX];
    output.extend(builder.finished_data().to_vec());

    output
}
