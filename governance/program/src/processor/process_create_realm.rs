//! Program state processor

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};

use crate::{
    state::{
        enums::GovernanceAccountType,
        realm::{get_governing_token_holding_address_seeds, get_realm_address_seeds, Realm},
    },
    tools::{account::create_and_serialize_account_signed, token::create_spl_token_account_signed},
};

/// process_create_realm
pub fn process_create_realm(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    name: String,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let realm_info = next_account_info(account_info_iter)?; // 1
    let governance_token_mint_info = next_account_info(account_info_iter)?; // 2
    let governance_token_holding_info = next_account_info(account_info_iter)?; // 3
    let payer_info = next_account_info(account_info_iter)?; // 4
    let system_info = next_account_info(account_info_iter)?; // 5
    let spl_token_info = next_account_info(account_info_iter)?; // 6
    let rent_sysvar_info = next_account_info(account_info_iter)?; // 7

    let mut council_token_mint_address = Option::<Pubkey>::None;

    // 8
    if let Ok(council_token_mint_info) = next_account_info(account_info_iter) {
        council_token_mint_address = Some(*council_token_mint_info.key);

        let council_token_holding_info = next_account_info(account_info_iter)?; //9

        create_spl_token_account_signed(
            payer_info,
            council_token_holding_info,
            get_governing_token_holding_address_seeds(realm_info.key, council_token_mint_info.key),
            council_token_mint_info,
            realm_info,
            program_id,
            system_info,
            spl_token_info,
            rent_sysvar_info,
        )?;
    }

    create_spl_token_account_signed(
        payer_info,
        governance_token_holding_info,
        get_governing_token_holding_address_seeds(realm_info.key, governance_token_mint_info.key),
        governance_token_mint_info,
        realm_info,
        program_id,
        system_info,
        spl_token_info,
        rent_sysvar_info,
    )?;

    let realm_data = Realm {
        account_type: GovernanceAccountType::Realm,
        governance_mint: *governance_token_mint_info.key,
        council_mint: council_token_mint_address,
        name: name.clone(),
    };

    create_and_serialize_account_signed::<Realm>(
        payer_info,
        &realm_info,
        &realm_data,
        get_realm_address_seeds(&name),
        program_id,
        system_info,
    )?;

    Ok(())
}