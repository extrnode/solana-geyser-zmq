//! FlatBuffer serialization module
use account_info_generated::account_info::{
    AccountInfo, AccountInfoArgs, Pubkey as AccountInfoPubkey, PubkeyArgs as AccountInfoPubkeyArgs,
};
use flatbuffers::FlatBufferBuilder;
use solana_geyser_plugin_interface::geyser_plugin_interface::{ReplicaAccountInfoV2, SlotStatus};
pub use solana_program::pubkey::Pubkey;

use self::slot_generated::slot::{Slot, SlotArgs};

#[allow(clippy::all)]
mod account_info_generated;
#[allow(clippy::all)]
mod slot_generated;

/// Struct which implements FlatBuffer serialization for accounts, block metadata and transactions data
#[derive(Debug, Copy, Clone)]
pub struct FlatBufferSerialization {}

const BYTE_PREFIX_ACCOUNT: u8 = 0;
const BYTE_PREFIX_SLOT: u8 = 1;

impl FlatBufferSerialization {
    pub fn serialize_account<'a>(
        &self,
        account: &'a ReplicaAccountInfoV2<'a>,
        slot: u64,
        is_startup: bool,
    ) -> Vec<u8> {
        let key = Pubkey::new_from_array(account.pubkey.try_into().unwrap());
        let owner = Pubkey::new_from_array(account.owner.try_into().unwrap());

        let mut builder = FlatBufferBuilder::new();

        let pubkey_vec = builder.create_vector(key.as_ref());
        let owner_vec = builder.create_vector(owner.as_ref());

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
                slot: slot,
                is_startup: is_startup,
            },
        );

        builder.finish(account_info, None);

        let mut output = vec![BYTE_PREFIX_ACCOUNT];
        output.extend(builder.finished_data().to_vec());

        output
    }

    pub fn serialize_slot<'a>(&self, slot: u64, status: SlotStatus) -> Vec<u8> {
        let mut builder = FlatBufferBuilder::new();

        let s = Slot::create(
            &mut builder,
            &SlotArgs {
                slot,
                status: match status {
                    SlotStatus::Processed => 0,
                    SlotStatus::Rooted => 1,
                    SlotStatus::Confirmed => 2,
                },
            },
        );

        builder.finish(s, None);

        let mut output = vec![BYTE_PREFIX_SLOT];
        output.extend(builder.finished_data().to_vec());

        output
    }
}
