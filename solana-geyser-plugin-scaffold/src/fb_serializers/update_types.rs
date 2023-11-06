use solana_geyser_plugin_interface::geyser_plugin_interface::{
    ReplicaAccountInfoVersions, ReplicaBlockInfoVersions, ReplicaTransactionInfoVersions,
};
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signature;

const BPF_LOADER_WRITE_INSTRUCTION_FIRST_BYTE: u8 = 0;
const BPF_UPGRADEABLE_LOADER_WRITE_INSTRUCTION_FIRST_BYTE: u8 = 1;

const NFT_KEYS: &[&str] = &[
    "BGUMAp9Gq7iTEuizy4pqaxsTyUCBK68MDfK752saRPUY", // bubblegum
    "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s",  // mpl token metadata
    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",  // spl token
    "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",  // spl token 2022
];

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

impl AccountUpdate {
    pub fn from_account(
        account: ReplicaAccountInfoVersions,
        slot: u64,
        is_startup: bool,
    ) -> anyhow::Result<Self> {
        match account {
            ReplicaAccountInfoVersions::V0_0_1(acc) => {
                let key = Pubkey::new_from_array(acc.pubkey.try_into()?);
                let owner = Pubkey::new_from_array(acc.owner.try_into()?);

                Ok(AccountUpdate {
                    key,
                    lamports: acc.lamports,
                    owner,
                    executable: acc.executable,
                    rent_epoch: acc.rent_epoch,
                    data: acc.data.to_vec(),
                    write_version: acc.write_version,
                    slot,
                    is_startup,
                })
            }
            ReplicaAccountInfoVersions::V0_0_2(acc) => {
                let key = Pubkey::new_from_array(acc.pubkey.try_into()?);
                let owner = Pubkey::new_from_array(acc.owner.try_into()?);

                Ok(AccountUpdate {
                    key,
                    lamports: acc.lamports,
                    owner,
                    executable: acc.executable,
                    rent_epoch: acc.rent_epoch,
                    data: acc.data.to_vec(),
                    write_version: acc.write_version,
                    slot,
                    is_startup,
                })
            }
            ReplicaAccountInfoVersions::V0_0_3(acc) => {
                let key = Pubkey::new_from_array(acc.pubkey.try_into()?);
                let owner = Pubkey::new_from_array(acc.owner.try_into()?);

                Ok(AccountUpdate {
                    key,
                    lamports: acc.lamports,
                    owner,
                    executable: acc.executable,
                    rent_epoch: acc.rent_epoch,
                    data: acc.data.to_vec(),
                    write_version: acc.write_version,
                    slot,
                    is_startup,
                })
            }
        }
    }

    pub fn is_nft_account(&self) -> bool {
        if NFT_KEYS.contains(&self.key.to_string().as_str()) {
            return true;
        }

        NFT_KEYS.contains(&self.owner.to_string().as_str())
    }
}

pub struct TransactionUpdate {
    pub signature: Signature,
    pub is_vote: bool,
    pub slot: u64,
    pub transaction: solana_sdk::transaction::SanitizedTransaction,
    pub transaction_meta: solana_transaction_status::TransactionStatusMeta,
    pub index: Option<usize>,
}

impl TransactionUpdate {
    pub fn from_transaction(tx: ReplicaTransactionInfoVersions, slot: u64) -> Self {
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

    pub fn is_deploy_tx(&self) -> bool {
        if self.transaction.message().instructions().len() != 1 {
            return false;
        }

        let instruction = self.transaction.message().instructions().iter().next();
        if instruction.is_none() {
            return false;
        }

        let instruction = instruction.unwrap();
        if instruction.data.is_empty() {
            return false;
        }

        let account_keys = self.transaction.message().account_keys();
        let program_id = account_keys[instruction.program_id_index as usize];
        if program_id == solana_sdk::bpf_loader::id() {
            return instruction.data[0] == BPF_LOADER_WRITE_INSTRUCTION_FIRST_BYTE;
        }

        if program_id == solana_sdk::bpf_loader_upgradeable::id() {
            return instruction.data[0] == BPF_UPGRADEABLE_LOADER_WRITE_INSTRUCTION_FIRST_BYTE;
        }

        false
    }

    pub fn is_nft_transaction(&self) -> bool {
        let account_keys = self.transaction.message().account_keys();
        for account_key in account_keys.iter() {
            if NFT_KEYS.contains(&account_key.to_string().as_str()) {
                return true;
            }
        }

        false
    }
}

pub struct BlockUpdate<'a> {
    pub parent_slot: Option<u64>,
    pub parent_blockhash: Option<&'a str>,
    pub slot: u64,
    pub blockhash: &'a str,
    pub rewards: &'a [solana_transaction_status::Reward],
    pub block_time: Option<i64>,
    pub block_height: Option<u64>,
    pub executed_transaction_count: Option<u64>,
}

impl<'a> From<ReplicaBlockInfoVersions<'a>> for BlockUpdate<'a> {
    fn from(block_info: ReplicaBlockInfoVersions<'a>) -> Self {
        match block_info {
            ReplicaBlockInfoVersions::V0_0_1(block) => BlockUpdate {
                parent_slot: None,
                parent_blockhash: None,
                slot: block.slot,
                blockhash: block.blockhash,
                rewards: block.rewards,
                block_time: block.block_time,
                block_height: block.block_height,
                executed_transaction_count: None,
            },
            ReplicaBlockInfoVersions::V0_0_2(block) => BlockUpdate {
                parent_slot: Some(block.parent_slot),
                parent_blockhash: Some(block.parent_blockhash),
                slot: block.slot,
                blockhash: block.blockhash,
                rewards: block.rewards,
                block_time: block.block_time,
                block_height: block.block_height,
                executed_transaction_count: Some(block.executed_transaction_count),
            },
        }
    }
}
