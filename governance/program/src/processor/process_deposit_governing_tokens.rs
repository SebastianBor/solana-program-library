//! Program state processor

use borsh::BorshSerialize;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};

use crate::{
    error::GovernanceError,
    state::{
        enums::{GovernanceAccountType, GoverningTokenType},
        realm::deserialize_realm,
        voter_record::{deserialize_voter_record, get_voter_record_address_seeds, VoterRecord},
    },
    tools::{
        account::create_and_serialize_account_signed,
        token::{get_amount_from_token_account, get_mint_from_token_account, transfer_spl_tokens},
    },
};

/// process deposit governing tokens
pub fn process_deposit_governing_tokens(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let realm_info = next_account_info(account_info_iter)?; // 0
    let governing_token_holding_info = next_account_info(account_info_iter)?; // 1
    let governing_token_source_info = next_account_info(account_info_iter)?; // 2
    let governing_token_owner_info = next_account_info(account_info_iter)?; // 3
    let voter_record_info = next_account_info(account_info_iter)?; // 4
    let payer_info = next_account_info(account_info_iter)?; // 5
    let system_info = next_account_info(account_info_iter)?; // 6
    let spl_token_info = next_account_info(account_info_iter)?; // 7

    let realm_data = deserialize_realm(realm_info)?;
    let governing_token_mint = get_mint_from_token_account(governing_token_holding_info)?;

    let governing_token_type = if governing_token_mint == realm_data.governance_mint {
        GoverningTokenType::Governance
    } else if Some(governing_token_mint) == realm_data.council_mint {
        GoverningTokenType::Council
    } else {
        return Err(GovernanceError::InvalidGoverningTokenMint.into());
    };

    let amount = get_amount_from_token_account(governing_token_source_info)?;

    transfer_spl_tokens(
        &governing_token_source_info,
        &governing_token_holding_info,
        &governing_token_owner_info,
        amount,
        spl_token_info,
    )?;

    let voter_record_address_seeds = get_voter_record_address_seeds(
        realm_info.key,
        &governing_token_mint,
        governing_token_owner_info.key,
    );

    if voter_record_info.data_len() == 0 {
        let voter_record_data = VoterRecord {
            account_type: GovernanceAccountType::VoterRecord,
            realm: *realm_info.key,
            token_owner: *governing_token_owner_info.key,
            token_deposit_amount: amount,
            token_type: governing_token_type,
            vote_authority: *governing_token_owner_info.key,
            active_votes_count: 0,
            total_votes_count: 0,
        };

        create_and_serialize_account_signed(
            payer_info,
            voter_record_info,
            &voter_record_data,
            voter_record_address_seeds,
            program_id,
            system_info,
        )?;
    } else {
        let mut voter_record_data =
            deserialize_voter_record(voter_record_info, voter_record_address_seeds)?;

        voter_record_data.token_deposit_amount = voter_record_data
            .token_deposit_amount
            .checked_add(amount)
            .unwrap();

        voter_record_data.serialize(&mut *voter_record_info.data.borrow_mut())?;
    }

    Ok(())
}