//! FlatBuffer serialization module
use crate::flatbuffer::account_info_generated::account_info::{AccountInfo, AccountInfoArgs};
use crate::flatbuffer::transaction_info_generated::transaction_info::{
    LoadedAddressesString, LoadedAddressesStringArgs, TransactionReturnData,
    TransactionReturnDataArgs, UiTokenAmountPtr, UiTokenAmountPtrArgs,
};
use crate::{
    errors::GeyserError,
    flatbuffer::transaction_info_generated::transaction_info::{
        CompiledInstruction, CompiledInstructionArgs, InnerByte, InnerByteArgs, InnerInstructions,
        InnerInstructionsArgs, InstructionError, InstructionErrorArgs, InstructionErrorData,
        InstructionErrorDataArgs, InstructionErrorInnerData, InstructionErrorType, StringValue,
        StringValueArgs, TransactionError, TransactionErrorArgs, TransactionErrorData,
        TransactionErrorType, TransactionInfo, TransactionInfoArgs, TransactionStatusMeta,
        TransactionStatusMetaArgs, TransactionTokenBalance, TransactionTokenBalanceArgs,
        UiTokenAmount, UiTokenAmountArgs, Uint32Value, Uint32ValueArgs,
    },
};
use flatbuffers::FlatBufferBuilder;
use solana_geyser_plugin_interface::geyser_plugin_interface::{ReplicaBlockInfo, SlotStatus};
pub use solana_program::hash::Hash;
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_transaction_status::{UiReturnDataEncoding, UiTransactionReturnData};

use self::account_data_generated::account_data::{AccountData, AccountDataArgs};
use self::metadata_generated::metadata::{Metadata, MetadataArgs};
use self::{
    block_info_generated::block_info::{BlockInfo, BlockInfoArgs},
    common_generated::common::{Reward, RewardArgs, RewardType},
    slot_generated::slot::{Slot, SlotArgs, Status},
};

#[allow(dead_code, clippy::all)]
pub mod account_data_generated;
#[allow(dead_code, clippy::all)]
pub mod account_info_generated;
#[allow(dead_code, clippy::all)]
mod block_info_generated;
#[allow(dead_code, clippy::all)]
mod common_generated;
#[allow(dead_code, clippy::all)]
mod metadata_generated;
#[allow(dead_code, clippy::all)]
mod slot_generated;
#[allow(dead_code, clippy::all)]
pub mod transaction_info_generated;
/// Struct which implements FlatBuffer serialization for accounts, block metadata and transactions data
#[derive(Debug, Copy, Clone)]
pub struct FlatBufferSerialization {}

pub const BYTE_PREFIX_ACCOUNT: u8 = 0;
const BYTE_PREFIX_SLOT: u8 = 1;
pub const BYTE_PREFIX_TX: u8 = 2;
const BYTE_PREFIX_BLOCK: u8 = 3;
const BYTE_PREFIX_METADATA: u8 = 4;

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

    let mut output = vec![BYTE_PREFIX_ACCOUNT];
    output.extend(builder.finished_data().to_vec());

    output
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

    let mut output = vec![BYTE_PREFIX_SLOT];
    output.extend(builder.finished_data().to_vec());

    output
}

pub fn serialize_block(block: &ReplicaBlockInfo) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();

    let rewards = if !block.rewards.is_empty() {
        let mut rewards_vec = Vec::with_capacity(block.rewards.len());
        for reward in block.rewards {
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

            rewards_vec.push(Reward::create(
                &mut builder,
                &RewardArgs {
                    pubkey,
                    lamports: reward.lamports,
                    post_balance: reward.post_balance,
                    reward_type,
                    commission: reward.commission,
                },
            ));
        }

        Some(builder.create_vector(rewards_vec.as_ref()))
    } else {
        None
    };

    let blockhash = builder.create_string(block.blockhash);

    let b = BlockInfo::create(
        &mut builder,
        &BlockInfoArgs {
            slot: block.slot,
            blockhash: Some(blockhash),
            block_time: block.block_time.unwrap_or(0),
            block_height: block.block_height.unwrap_or(0),
            rewards,
        },
    );

    builder.finish(b, None);

    let mut output = vec![BYTE_PREFIX_BLOCK];
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
    pub index: Option<usize>,
}

pub fn serialize_transaction(transaction: &TransactionUpdate) -> Result<Vec<u8>, GeyserError> {
    let mut builder = FlatBufferBuilder::new();

    let loaded_addresses_string = match &transaction.transaction.message() {
        solana_sdk::message::SanitizedMessage::Legacy(_) => None,
        solana_sdk::message::SanitizedMessage::V0(loaded_message_v0) => {
            let writable = loaded_message_v0
                .loaded_addresses
                .writable
                .iter()
                .map(|key| builder.create_string(key.to_string().as_str()))
                .collect::<Vec<_>>();
            let writable = Some(builder.create_vector(writable.as_ref()));

            let readonly = loaded_message_v0
                .loaded_addresses
                .readonly
                .iter()
                .map(|key| builder.create_string(key.to_string().as_str()))
                .collect::<Vec<_>>();
            let readonly = Some(builder.create_vector(readonly.as_ref()));

            let loaded_addresses = LoadedAddressesString::create(
                &mut builder,
                &LoadedAddressesStringArgs { writable, readonly },
            );

            Some(loaded_addresses)
        }
    };

    let tx = bincode::serialize(&transaction.transaction.to_versioned_transaction())
        .map_err(|_| GeyserError::TxSerializeError)?;

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

    let (pre_token_balances, pre_token_balances_ptr) = if let Some(pre_token_balances) =
        &transaction.transaction_meta.pre_token_balances
    {
        let mut pre_token_balances_vec = Vec::with_capacity(pre_token_balances.len());
        let mut pre_token_balances_ptr_vec = Vec::with_capacity(pre_token_balances.len());

        for transaction_token_balance in pre_token_balances {
            let amount =
                Some(builder.create_string(&transaction_token_balance.ui_token_amount.amount));
            let ui_amount_string = Some(
                builder.create_string(&transaction_token_balance.ui_token_amount.ui_amount_string),
            );
            let decimals = transaction_token_balance.ui_token_amount.decimals;

            let ui_token_amount = Some(UiTokenAmount::create(
                &mut builder,
                &UiTokenAmountArgs {
                    ui_amount: 0.0,
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

            pre_token_balances_ptr_vec.push(UiTokenAmountPtr::create(
                &mut builder,
                &UiTokenAmountPtrArgs {
                    amount: transaction_token_balance.ui_token_amount.ui_amount,
                },
            ));
        }
        (
            Some(builder.create_vector(pre_token_balances_vec.as_ref())),
            Some(builder.create_vector(pre_token_balances_ptr_vec.as_ref())),
        )
    } else {
        (None, None)
    };

    let (post_token_balances, post_token_balances_ptr) = if let Some(post_token_balances) =
        &transaction.transaction_meta.post_token_balances
    {
        let mut post_token_balances_vec = Vec::with_capacity(post_token_balances.len());
        let mut post_token_balances_ptr_vec = Vec::with_capacity(post_token_balances.len());

        for transaction_token_balance in post_token_balances {
            let amount =
                Some(builder.create_string(&transaction_token_balance.ui_token_amount.amount));
            let ui_amount_string = Some(
                builder.create_string(&transaction_token_balance.ui_token_amount.ui_amount_string),
            );
            let decimals = transaction_token_balance.ui_token_amount.decimals;

            let ui_token_amount = Some(UiTokenAmount::create(
                &mut builder,
                &UiTokenAmountArgs {
                    ui_amount: 0.0,
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

            post_token_balances_ptr_vec.push(UiTokenAmountPtr::create(
                &mut builder,
                &UiTokenAmountPtrArgs {
                    amount: transaction_token_balance.ui_token_amount.ui_amount,
                },
            ));
        }
        (
            Some(builder.create_vector(post_token_balances_vec.as_ref())),
            Some(builder.create_vector(post_token_balances_ptr_vec.as_ref())),
        )
    } else {
        (None, None)
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

            rewards_vec.push(Reward::create(
                &mut builder,
                &RewardArgs {
                    pubkey,
                    lamports: reward.lamports,
                    post_balance: reward.post_balance,
                    reward_type,
                    commission: reward.commission,
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

    let status =
        if transaction.transaction_meta.status.is_ok() {
            None
        } else {
            match transaction.transaction_meta.status.clone().err().unwrap() {
                solana_sdk::transaction::TransactionError::AccountInUse => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::AccountInUse,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::AccountLoadedTwice => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::AccountLoadedTwice,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::AccountNotFound => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::AccountNotFound,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::ProgramAccountNotFound => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::ProgramAccountNotFound,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::InsufficientFundsForFee => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::InsufficientFundsForFee,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::InvalidAccountForFee => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::InvalidAccountForFee,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::AlreadyProcessed => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::AlreadyProcessed,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::BlockhashNotFound => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::BlockhashNotFound,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::InstructionError(index, error) => {
                    let inner_instruction = match error {
                        solana_sdk::instruction::InstructionError::GenericError =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::GenericError,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                }
                            )),
                        solana_sdk::instruction::InstructionError::InvalidArgument =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::InvalidArgument,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::InvalidInstructionData =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::InvalidInstructionData,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::InvalidAccountData =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::InvalidAccountData,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::AccountDataTooSmall =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::AccountDataTooSmall,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::InsufficientFunds =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::InsufficientFunds,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::IncorrectProgramId =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::IncorrectProgramId,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::MissingRequiredSignature =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::MissingRequiredSignature,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::AccountAlreadyInitialized =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::AccountAlreadyInitialized,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::UninitializedAccount =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::UninitializedAccount,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::UnbalancedInstruction =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::UnbalancedInstruction,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::ModifiedProgramId =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::ModifiedProgramId,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::ExternalAccountLamportSpend =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::ExternalAccountLamportSpend,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::ExternalAccountDataModified =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::ExternalAccountDataModified,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::ReadonlyLamportChange =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::ReadonlyLamportChange,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::ReadonlyDataModified =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::ReadonlyDataModified,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::DuplicateAccountIndex =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::DuplicateAccountIndex,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::ExecutableModified =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::ExecutableModified,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::RentEpochModified =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::RentEpochModified,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::NotEnoughAccountKeys =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::NotEnoughAccountKeys,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::AccountDataSizeChanged =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::AccountDataSizeChanged,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::AccountNotExecutable =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::AccountNotExecutable,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::AccountBorrowFailed =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::AccountBorrowFailed,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::AccountBorrowOutstanding =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::AccountBorrowOutstanding,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::DuplicateAccountOutOfSync =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::DuplicateAccountOutOfSync,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::Custom(error_code) => {
                            let val = Some(Uint32Value::create(
                                &mut builder,
                                &Uint32ValueArgs {
                                    value: error_code,
                                }
                            ).as_union_value());
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::Custom,
                                    err_data_type: InstructionErrorInnerData::Custom,
                                    err_data: val,
                                },
                            ))
                        },
                        solana_sdk::instruction::InstructionError::InvalidError =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::InvalidError,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::ExecutableDataModified =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::ExecutableDataModified,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::ExecutableLamportChange =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::ExecutableLamportChange,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::ExecutableAccountNotRentExempt =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::ExecutableAccountNotRentExempt,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::UnsupportedProgramId =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::UnsupportedProgramId,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::CallDepth =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::CallDepth,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::MissingAccount =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::MissingAccount,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::ReentrancyNotAllowed =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::ReentrancyNotAllowed,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::MaxSeedLengthExceeded =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::MaxSeedLengthExceeded,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::InvalidSeeds =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::InvalidSeeds,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::InvalidRealloc =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::InvalidRealloc,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::ComputationalBudgetExceeded =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::ComputationalBudgetExceeded,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::PrivilegeEscalation =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::PrivilegeEscalation,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::ProgramEnvironmentSetupFailure =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::ProgramEnvironmentSetupFailure,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::ProgramFailedToComplete =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::ProgramFailedToComplete,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::ProgramFailedToCompile =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::ProgramFailedToCompile,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::Immutable =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::Immutable,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::IncorrectAuthority =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::IncorrectAuthority,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::BorshIoError(msg) => {
                            let m = Some(builder.create_string(&msg));
                            let val = Some(StringValue::create(
                                &mut builder,
                                &StringValueArgs {
                                    value: m,
                                }
                            ).as_union_value());

                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::BorshIoError,
                                    err_data_type: InstructionErrorInnerData::BorshIoError,
                                    err_data: val,
                                },
                            ))
                        }
                        solana_sdk::instruction::InstructionError::AccountNotRentExempt =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::AccountNotRentExempt,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::InvalidAccountOwner =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::InvalidAccountOwner,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::ArithmeticOverflow =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::ArithmeticOverflow,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::UnsupportedSysvar =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::UnsupportedSysvar,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::IllegalOwner =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::IllegalOwner,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::MaxAccountsDataSizeExceeded =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::MaxAccountsDataSizeExceeded,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                        solana_sdk::instruction::InstructionError::MaxAccountsExceeded =>
                            Some(InstructionError::create(
                                &mut builder,
                                &InstructionErrorArgs {
                                    err_type: InstructionErrorType::MaxAccountsExceeded,
                                    err_data_type: Default::default(),
                                    err_data: None,
                                },
                            )),
                    };
                    let inner_error_data = Some(
                        InstructionErrorData::create(
                            &mut builder,
                            &InstructionErrorDataArgs {
                                instruction_number: index,
                                err: inner_instruction,
                            },
                        )
                        .as_union_value(),
                    );

                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::InstructionError,
                            err_data_type: TransactionErrorData::InstructionError,
                            err_data: inner_error_data,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::CallChainTooDeep => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::CallChainTooDeep,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::MissingSignatureForFee => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::MissingSignatureForFee,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::InvalidAccountIndex => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::InvalidAccountIndex,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::SignatureFailure => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::SignatureFailure,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::InvalidProgramForExecution => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::InvalidProgramForExecution,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::SanitizeFailure => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::SanitizeFailure,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::ClusterMaintenance => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::ClusterMaintenance,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::AccountBorrowOutstanding => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::AccountBorrowOutstanding,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::WouldExceedMaxBlockCostLimit => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::WouldExceedMaxBlockCostLimit,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::UnsupportedVersion => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::UnsupportedVersion,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::InvalidWritableAccount => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::InvalidWritableAccount,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::WouldExceedMaxAccountCostLimit => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::WouldExceedMaxAccountCostLimit,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::WouldExceedAccountDataBlockLimit => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::WouldExceedAccountDataBlockLimit,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::TooManyAccountLocks => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::TooManyAccountLocks,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::AddressLookupTableNotFound => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::AddressLookupTableNotFound,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::InvalidAddressLookupTableOwner => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::InvalidAddressLookupTableOwner,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::InvalidAddressLookupTableData => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::InvalidAddressLookupTableData,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::InvalidAddressLookupTableIndex => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::InvalidAddressLookupTableIndex,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::InvalidRentPayingAccount => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::InvalidRentPayingAccount,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::WouldExceedMaxVoteCostLimit => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::WouldExceedMaxVoteCostLimit,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::WouldExceedAccountDataTotalLimit => {
                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::WouldExceedAccountDataTotalLimit,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::DuplicateInstruction(index) => {
                    let val = Some(
                        InnerByte::create(&mut builder, &InnerByteArgs { inner_byte: index })
                            .as_union_value(),
                    );

                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::DuplicateInstruction,
                            err_data_type: TransactionErrorData::InnerByte,
                            err_data: val,
                        },
                    ))
                }
                solana_sdk::transaction::TransactionError::InsufficientFundsForRent {
                    account_index,
                } => {
                    let val = Some(
                        InnerByte::create(
                            &mut builder,
                            &InnerByteArgs {
                                inner_byte: account_index,
                            },
                        )
                        .as_union_value(),
                    );

                    Some(TransactionError::create(
                        &mut builder,
                        &TransactionErrorArgs {
                            err_type: TransactionErrorType::InsufficientFundsForRent,
                            err_data_type: TransactionErrorData::InnerByte,
                            err_data: val,
                        },
                    ))
                }
            }
        };

    let transaction_meta = Some(TransactionStatusMeta::create(
        &mut builder,
        &TransactionStatusMetaArgs {
            status,
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

    let signature_string = Some(builder.create_string(transaction.signature.to_string().as_str()));
    let tx = Some(builder.create_vector(&tx));
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

    let return_data = transaction
        .transaction_meta
        .return_data
        .as_ref()
        .map(|return_data| UiTransactionReturnData::from(return_data.clone()));
    let return_data = match return_data {
        None => None,
        Some(return_data) => {
            let program_id = Some(builder.create_string(&return_data.program_id));
            let data_value = Some(builder.create_string(&return_data.data.0));
            let data_encoding = match return_data.data.1 {
                UiReturnDataEncoding::Base64 => {
                    transaction_info_generated::transaction_info::UiReturnDataEncoding::base64
                }
            };
            Some(TransactionReturnData::create(
                &mut builder,
                &TransactionReturnDataArgs {
                    program_id,
                    data_value,
                    data_encoding,
                },
            ))
        }
    };

    let transaction_info = TransactionInfo::create(
        &mut builder,
        &TransactionInfoArgs {
            signature_string,
            is_vote: transaction.is_vote,
            slot: transaction.slot,
            transaction: tx,
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
        },
    );
    builder.finish(transaction_info, None);

    let mut output = vec![BYTE_PREFIX_TX];
    output.extend(builder.finished_data().to_vec());

    Ok(output)
}

pub fn serialize_metadata(send_errors: u64) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();

    let obj = Metadata::create(&mut builder, &MetadataArgs { send_errors });
    builder.finish(obj, None);

    let mut output = vec![BYTE_PREFIX_METADATA];
    output.extend(builder.finished_data().to_vec());

    output
}
