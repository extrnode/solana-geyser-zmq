use flatbuffers::{FlatBufferBuilder, ForwardsUOffset, Vector, WIPOffset};
use solana_transaction_status::{Rewards, UiReturnDataEncoding, UiTransactionReturnData};
use utils::errors::GeyserError;
use utils::flatbuffer::common_generated::common::{Reward, RewardArgs, RewardType};
use utils::flatbuffer::transaction_info_generated;
use utils::flatbuffer::transaction_info_generated::transaction_info::{
    CompiledInstruction, CompiledInstructionArgs, InnerByte, InnerByteArgs, InnerInstructionV2,
    InnerInstructionV2Args, InnerInstructionsV2, InnerInstructionsV2Args, InstructionError,
    InstructionErrorArgs, InstructionErrorData, InstructionErrorDataArgs,
    InstructionErrorInnerData, InstructionErrorType, LoadedAddressesString,
    LoadedAddressesStringArgs, StringValue, StringValueArgs, TransactionError,
    TransactionErrorArgs, TransactionErrorData, TransactionErrorType, TransactionReturnData,
    TransactionReturnDataArgs, TransactionStatusMeta, TransactionStatusMetaArgs,
    TransactionTokenBalance, TransactionTokenBalanceArgs, UiTokenAmount, UiTokenAmountArgs,
    UiTokenAmountPtr, UiTokenAmountPtrArgs, Uint32Value, Uint32ValueArgs,
};

pub struct TxInfoArgs<'a> {
    pub transaction_serialized: Option<WIPOffset<Vector<'a, u8>>>,
    pub loaded_addresses_string: Option<WIPOffset<LoadedAddressesString<'a>>>,
    pub memo: Option<WIPOffset<&'a str>>,
    pub account_keys_string: Option<WIPOffset<Vector<'a, ForwardsUOffset<&'a str>>>>,
}

pub struct TxMetaArgs<'a> {
    pub meta: Option<WIPOffset<TransactionStatusMeta<'a>>>,
    pub pre_token_balances_ptr:
        Option<WIPOffset<Vector<'a, ForwardsUOffset<UiTokenAmountPtr<'a>>>>>,
    pub post_token_balances_ptr:
        Option<WIPOffset<Vector<'a, ForwardsUOffset<UiTokenAmountPtr<'a>>>>>,
    pub inner_instructions: Option<WIPOffset<Vector<'a, ForwardsUOffset<InnerInstructionsV2<'a>>>>>,
    pub return_data: Option<WIPOffset<TransactionReturnData<'a>>>,
}

pub fn extract_tx_meta_args<'bldr: 'args, 'args: 'mut_bldr, 'mut_bldr>(
    transaction_meta: &'args solana_transaction_status::TransactionStatusMeta,
    builder: &'mut_bldr mut FlatBufferBuilder<'bldr>,
) -> TxMetaArgs<'bldr> {
    let meta = extract_meta(transaction_meta, builder);

    let pre_token_balances_ptr =
        extract_token_balances_ptr(&transaction_meta.pre_token_balances, builder);
    let post_token_balances_ptr =
        extract_token_balances_ptr(&transaction_meta.post_token_balances, builder);

    let inner_instructions =
        extract_inner_instructions(&transaction_meta.inner_instructions, builder);
    let return_data = extract_return_data(&transaction_meta.return_data, builder);

    TxMetaArgs {
        meta,
        pre_token_balances_ptr,
        post_token_balances_ptr,
        inner_instructions,
        return_data,
    }
}

pub fn extract_tx_info_args<'bldr: 'args, 'args: 'mut_bldr, 'mut_bldr>(
    transaction: &'args solana_sdk::transaction::SanitizedTransaction,
    builder: &'mut_bldr mut FlatBufferBuilder<'bldr>,
) -> Result<TxInfoArgs<'bldr>, GeyserError> {
    let transaction_serialized = bincode::serialize(&transaction.to_versioned_transaction())
        .map_err(|_| GeyserError::TxSerializeError)?;
    let transaction_serialized = Some(builder.create_vector(&transaction_serialized));

    let loaded_addresses_string = extract_loaded_addresses_string(transaction.message(), builder);

    let memo =
        solana_transaction_status::extract_memos::extract_and_fmt_memos(transaction.message())
            .map(|m| builder.create_string(&m));

    let account_keys = transaction
        .message()
        .account_keys()
        .iter()
        .map(|key| builder.create_string(key.to_string().as_str()))
        .collect::<Vec<_>>();
    let account_keys_string = Some(builder.create_vector(account_keys.as_ref()));

    Ok(TxInfoArgs {
        transaction_serialized,
        loaded_addresses_string,
        memo,
        account_keys_string,
    })
}

pub fn extract_rewards<'bldr: 'args, 'args: 'mut_bldr, 'mut_bldr>(
    rewards: &'args Option<Rewards>,
    builder: &'mut_bldr mut FlatBufferBuilder<'bldr>,
) -> Option<WIPOffset<Vector<'bldr, ForwardsUOffset<Reward<'bldr>>>>> {
    if rewards.is_none() {
        return None;
    }
    let rewards = rewards.as_ref().unwrap();

    let mut rewards_vec = Vec::with_capacity(rewards.len());
    for reward in rewards {
        let pubkey = Some(builder.create_string(&reward.pubkey));

        rewards_vec.push(Reward::create(
            builder,
            &RewardArgs {
                pubkey,
                lamports: reward.lamports,
                post_balance: reward.post_balance,
                reward_type: extract_reward_type(reward.reward_type),
                commission: reward.commission,
            },
        ));
    }

    Some(builder.create_vector(rewards_vec.as_ref()))
}

fn extract_meta<'bldr: 'args, 'args: 'mut_bldr, 'mut_bldr>(
    meta: &'args solana_transaction_status::TransactionStatusMeta,
    builder: &'mut_bldr mut FlatBufferBuilder<'bldr>,
) -> Option<WIPOffset<TransactionStatusMeta<'bldr>>> {
    let pre_token_balances = extract_token_balances(&meta.pre_token_balances, builder);
    let post_token_balances = extract_token_balances(&meta.post_token_balances, builder);

    let rewards = extract_rewards(&meta.rewards, builder);

    let pre_balances = Some(builder.create_vector(meta.pre_balances.as_ref()));
    let post_balances = Some(builder.create_vector(meta.post_balances.as_ref()));

    let log_messages = if let Some(logs) = &meta.log_messages {
        let log_messages = logs
            .iter()
            .map(|log| builder.create_string(log))
            .collect::<Vec<_>>();
        Some(builder.create_vector(log_messages.as_ref()))
    } else {
        None
    };

    let status = extract_tx_status(&meta.status, builder);

    Some(TransactionStatusMeta::create(
        builder,
        &TransactionStatusMetaArgs {
            status,
            fee: meta.fee,
            pre_balances,
            post_balances,
            inner_instructions: None,
            log_messages,
            pre_token_balances,
            post_token_balances,
            rewards,
        },
    ))
}

fn extract_inner_instructions<'bldr: 'args, 'args: 'mut_bldr, 'mut_bldr>(
    inner_instructions: &'args Option<Vec<solana_transaction_status::InnerInstructions>>,
    builder: &'mut_bldr mut FlatBufferBuilder<'bldr>,
) -> Option<WIPOffset<Vector<'bldr, ForwardsUOffset<InnerInstructionsV2<'bldr>>>>> {
    if inner_instructions.is_none() {
        return None;
    }

    let inner_instructions = inner_instructions.as_ref().unwrap();
    let mut inner_instructions_vec = Vec::with_capacity(inner_instructions.len());

    for inner_instruction in inner_instructions {
        let mut inner_instruction_vec = Vec::with_capacity(inner_instruction.instructions.len());

        for instruction in &inner_instruction.instructions {
            let accounts = Some(builder.create_vector(instruction.instruction.accounts.as_ref()));
            let data = Some(builder.create_vector(instruction.instruction.data.as_ref()));
            let compiled_instruction = CompiledInstruction::create(
                builder,
                &CompiledInstructionArgs {
                    program_id_index: instruction.instruction.program_id_index,
                    accounts,
                    data,
                },
            );

            inner_instruction_vec.push(InnerInstructionV2::create(
                builder,
                &InnerInstructionV2Args {
                    instruction: Some(compiled_instruction),
                    stack_height: instruction.stack_height,
                },
            ))
        }
        let instructions = Some(builder.create_vector(inner_instruction_vec.as_ref()));

        inner_instructions_vec.push(InnerInstructionsV2::create(
            builder,
            &InnerInstructionsV2Args {
                index: inner_instruction.index,
                instructions,
            },
        ));
    }

    Some(builder.create_vector(inner_instructions_vec.as_ref()))
}

fn extract_token_balances_ptr<'bldr: 'args, 'args: 'mut_bldr, 'mut_bldr>(
    token_balance: &'args Option<Vec<solana_transaction_status::TransactionTokenBalance>>,
    builder: &'mut_bldr mut FlatBufferBuilder<'bldr>,
) -> Option<WIPOffset<Vector<'bldr, ForwardsUOffset<UiTokenAmountPtr<'bldr>>>>> {
    if token_balance.is_none() {
        return None;
    }

    let token_balance = token_balance.as_ref().unwrap();

    let mut token_balances_ptr_vec = Vec::with_capacity(token_balance.len());

    for transaction_token_balance in token_balance {
        token_balances_ptr_vec.push(UiTokenAmountPtr::create(
            builder,
            &UiTokenAmountPtrArgs {
                amount: transaction_token_balance.ui_token_amount.ui_amount,
            },
        ));
    }

    Some(builder.create_vector(token_balances_ptr_vec.as_ref()))
}

fn extract_return_data<'bldr: 'args, 'args: 'mut_bldr, 'mut_bldr>(
    return_data: &'args Option<solana_sdk::transaction_context::TransactionReturnData>,
    builder: &'mut_bldr mut FlatBufferBuilder<'bldr>,
) -> Option<WIPOffset<TransactionReturnData<'bldr>>> {
    let return_data = return_data
        .as_ref()
        .map(|return_data| UiTransactionReturnData::from(return_data.clone()));

    match return_data {
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
                builder,
                &TransactionReturnDataArgs {
                    program_id,
                    data_value,
                    data_encoding,
                },
            ))
        }
    }
}

fn extract_loaded_addresses_string<'bldr: 'args, 'args: 'mut_bldr, 'mut_bldr>(
    message: &'args solana_program::message::SanitizedMessage,
    builder: &'mut_bldr mut FlatBufferBuilder<'bldr>,
) -> Option<WIPOffset<LoadedAddressesString<'bldr>>> {
    match message {
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
                builder,
                &LoadedAddressesStringArgs { writable, readonly },
            );

            Some(loaded_addresses)
        }
    }
}

fn extract_reward_type(reward_type: Option<solana_sdk::reward_type::RewardType>) -> RewardType {
    if reward_type.is_none() {
        return RewardType::None;
    }

    let reward_type = reward_type.unwrap();

    match reward_type {
        solana_transaction_status::RewardType::Fee => RewardType::Fee,
        solana_transaction_status::RewardType::Rent => RewardType::Rent,
        solana_transaction_status::RewardType::Staking => RewardType::Staking,
        solana_transaction_status::RewardType::Voting => RewardType::Voting,
    }
}

fn extract_token_balances<'bldr: 'args, 'args: 'mut_bldr, 'mut_bldr>(
    token_balance: &'args Option<Vec<solana_transaction_status::TransactionTokenBalance>>,
    builder: &'mut_bldr mut FlatBufferBuilder<'bldr>,
) -> Option<WIPOffset<Vector<'bldr, ForwardsUOffset<TransactionTokenBalance<'bldr>>>>> {
    if token_balance.is_none() {
        return None;
    }

    let token_balance = token_balance.as_ref().unwrap();

    let mut token_balances_vec = Vec::with_capacity(token_balance.len());

    for transaction_token_balance in token_balance {
        let amount = Some(builder.create_string(&transaction_token_balance.ui_token_amount.amount));
        let ui_amount_string = Some(
            builder.create_string(&transaction_token_balance.ui_token_amount.ui_amount_string),
        );
        let decimals = transaction_token_balance.ui_token_amount.decimals;

        let ui_token_amount = Some(UiTokenAmount::create(
            builder,
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

        token_balances_vec.push(TransactionTokenBalance::create(
            builder,
            &TransactionTokenBalanceArgs {
                account_index: transaction_token_balance.account_index,
                mint,
                ui_token_amount,
                owner,
                program_id,
            },
        ));
    }

    Some(builder.create_vector(token_balances_vec.as_ref()))
}

fn extract_tx_status<'bldr: 'args, 'args: 'mut_bldr, 'mut_bldr>(
    status: &'args solana_sdk::transaction::Result<()>,
    builder: &'mut_bldr mut FlatBufferBuilder<'bldr>,
) -> Option<WIPOffset<TransactionError<'bldr>>> {
    if status.is_ok() {
        return None;
    }

    match status.clone().err().unwrap() {
        solana_sdk::transaction::TransactionError::AccountInUse => Some(TransactionError::create(
            builder,
            &TransactionErrorArgs {
                err_type: TransactionErrorType::AccountInUse,
                err_data_type: Default::default(),
                err_data: None,
            },
        )),
        solana_sdk::transaction::TransactionError::AccountLoadedTwice => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::AccountLoadedTwice,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::AccountNotFound => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::AccountNotFound,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::ProgramAccountNotFound => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::ProgramAccountNotFound,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::InsufficientFundsForFee => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::InsufficientFundsForFee,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::InvalidAccountForFee => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::InvalidAccountForFee,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::AlreadyProcessed => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::AlreadyProcessed,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::BlockhashNotFound => {
            Some(TransactionError::create(
                builder,
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
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::GenericError,
                            err_data_type: Default::default(),
                            err_data: None,
                        }
                    )),
                solana_sdk::instruction::InstructionError::InvalidArgument =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::InvalidArgument,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::InvalidInstructionData =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::InvalidInstructionData,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::InvalidAccountData =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::InvalidAccountData,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::AccountDataTooSmall =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::AccountDataTooSmall,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::InsufficientFunds =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::InsufficientFunds,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::IncorrectProgramId =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::IncorrectProgramId,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::MissingRequiredSignature =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::MissingRequiredSignature,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::AccountAlreadyInitialized =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::AccountAlreadyInitialized,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::UninitializedAccount =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::UninitializedAccount,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::UnbalancedInstruction =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::UnbalancedInstruction,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::ModifiedProgramId =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::ModifiedProgramId,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::ExternalAccountLamportSpend =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::ExternalAccountLamportSpend,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::ExternalAccountDataModified =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::ExternalAccountDataModified,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::ReadonlyLamportChange =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::ReadonlyLamportChange,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::ReadonlyDataModified =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::ReadonlyDataModified,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::DuplicateAccountIndex =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::DuplicateAccountIndex,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::ExecutableModified =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::ExecutableModified,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::RentEpochModified =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::RentEpochModified,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::NotEnoughAccountKeys =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::NotEnoughAccountKeys,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::AccountDataSizeChanged =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::AccountDataSizeChanged,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::AccountNotExecutable =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::AccountNotExecutable,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::AccountBorrowFailed =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::AccountBorrowFailed,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::AccountBorrowOutstanding =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::AccountBorrowOutstanding,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::DuplicateAccountOutOfSync =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::DuplicateAccountOutOfSync,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::Custom(error_code) => {
                    let val = Some(Uint32Value::create(
                        builder,
                        &Uint32ValueArgs {
                            value: error_code,
                        }
                    ).as_union_value());
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::Custom,
                            err_data_type: InstructionErrorInnerData::Custom,
                            err_data: val,
                        },
                    ))
                },
                solana_sdk::instruction::InstructionError::InvalidError =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::InvalidError,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::ExecutableDataModified =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::ExecutableDataModified,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::ExecutableLamportChange =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::ExecutableLamportChange,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::ExecutableAccountNotRentExempt =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::ExecutableAccountNotRentExempt,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::UnsupportedProgramId =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::UnsupportedProgramId,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::CallDepth =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::CallDepth,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::MissingAccount =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::MissingAccount,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::ReentrancyNotAllowed =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::ReentrancyNotAllowed,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::MaxSeedLengthExceeded =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::MaxSeedLengthExceeded,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::InvalidSeeds =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::InvalidSeeds,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::InvalidRealloc =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::InvalidRealloc,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::ComputationalBudgetExceeded =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::ComputationalBudgetExceeded,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::PrivilegeEscalation =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::PrivilegeEscalation,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::ProgramEnvironmentSetupFailure =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::ProgramEnvironmentSetupFailure,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::ProgramFailedToComplete =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::ProgramFailedToComplete,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::ProgramFailedToCompile =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::ProgramFailedToCompile,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::Immutable =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::Immutable,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::IncorrectAuthority =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::IncorrectAuthority,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::BorshIoError(msg) => {
                    let m = Some(builder.create_string(&msg));
                    let val = Some(StringValue::create(
                        builder,
                        &StringValueArgs {
                            value: m,
                        }
                    ).as_union_value());

                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::BorshIoError,
                            err_data_type: InstructionErrorInnerData::BorshIoError,
                            err_data: val,
                        },
                    ))
                }
                solana_sdk::instruction::InstructionError::AccountNotRentExempt =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::AccountNotRentExempt,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::InvalidAccountOwner =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::InvalidAccountOwner,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::ArithmeticOverflow =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::ArithmeticOverflow,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::UnsupportedSysvar =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::UnsupportedSysvar,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::IllegalOwner =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::IllegalOwner,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::MaxAccountsDataAllocationsExceeded =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::MaxAccountsDataAllocationsExceeded,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::MaxAccountsExceeded =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::MaxAccountsExceeded,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::MaxInstructionTraceLengthExceeded =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::MaxInstructionTraceLengthExceeded,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
                solana_sdk::instruction::InstructionError::BuiltinProgramsMustConsumeComputeUnits =>
                    Some(InstructionError::create(
                        builder,
                        &InstructionErrorArgs {
                            err_type: InstructionErrorType::BuiltinProgramsMustConsumeComputeUnits,
                            err_data_type: Default::default(),
                            err_data: None,
                        },
                    )),
            };
            let inner_error_data = Some(
                InstructionErrorData::create(
                    builder,
                    &InstructionErrorDataArgs {
                        instruction_number: index,
                        err: inner_instruction,
                    },
                )
                .as_union_value(),
            );

            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::InstructionError,
                    err_data_type: TransactionErrorData::InstructionError,
                    err_data: inner_error_data,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::CallChainTooDeep => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::CallChainTooDeep,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::MissingSignatureForFee => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::MissingSignatureForFee,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::InvalidAccountIndex => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::InvalidAccountIndex,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::SignatureFailure => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::SignatureFailure,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::InvalidProgramForExecution => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::InvalidProgramForExecution,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::SanitizeFailure => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::SanitizeFailure,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::ClusterMaintenance => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::ClusterMaintenance,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::AccountBorrowOutstanding => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::AccountBorrowOutstanding,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::WouldExceedMaxBlockCostLimit => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::WouldExceedMaxBlockCostLimit,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::UnsupportedVersion => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::UnsupportedVersion,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::InvalidWritableAccount => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::InvalidWritableAccount,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::WouldExceedMaxAccountCostLimit => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::WouldExceedMaxAccountCostLimit,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::WouldExceedAccountDataBlockLimit => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::WouldExceedAccountDataBlockLimit,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::TooManyAccountLocks => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::TooManyAccountLocks,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::AddressLookupTableNotFound => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::AddressLookupTableNotFound,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::InvalidAddressLookupTableOwner => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::InvalidAddressLookupTableOwner,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::InvalidAddressLookupTableData => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::InvalidAddressLookupTableData,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::InvalidAddressLookupTableIndex => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::InvalidAddressLookupTableIndex,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::InvalidRentPayingAccount => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::InvalidRentPayingAccount,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::WouldExceedMaxVoteCostLimit => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::WouldExceedMaxVoteCostLimit,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::WouldExceedAccountDataTotalLimit => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::WouldExceedAccountDataTotalLimit,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::DuplicateInstruction(index) => {
            let val = Some(
                InnerByte::create(builder, &InnerByteArgs { inner_byte: index }).as_union_value(),
            );

            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::DuplicateInstruction,
                    err_data_type: TransactionErrorData::InnerByte,
                    err_data: val,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::InsufficientFundsForRent { account_index } => {
            let val = Some(
                InnerByte::create(
                    builder,
                    &InnerByteArgs {
                        inner_byte: account_index,
                    },
                )
                .as_union_value(),
            );

            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::InsufficientFundsForRent,
                    err_data_type: TransactionErrorData::InnerByte,
                    err_data: val,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::MaxLoadedAccountsDataSizeExceeded => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::MaxLoadedAccountsDataSizeExceeded,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::InvalidLoadedAccountsDataSizeLimit => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::InvalidLoadedAccountsDataSizeLimit,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::ResanitizationNeeded => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::ResanitizationNeeded,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
        solana_sdk::transaction::TransactionError::UnbalancedTransaction => {
            Some(TransactionError::create(
                builder,
                &TransactionErrorArgs {
                    err_type: TransactionErrorType::UnbalancedTransaction,
                    err_data_type: Default::default(),
                    err_data: None,
                },
            ))
        }
    }
}
