//! Program state processor

use crate::{error::TimelockError, state::{enums::TimelockType, timelock_config::TimelockConfig, timelock_program::TimelockProgram, timelock_set::{TimelockSet, TIMELOCK_SET_VERSION}, timelock_state::{DESC_SIZE, NAME_SIZE}}, utils::{
        assert_initialized, assert_rent_exempt, assert_same_version_as_program,
        assert_token_program_is_correct, assert_uninitialized, spl_token_mint_to,
        TokenMintToParams,
    }};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_pack::Pack,
    pubkey::Pubkey,
    sysvar::{rent::Rent, Sysvar},
};
use spl_token::state::{Account, Mint};

/// Create a new timelock set
pub fn process_init_timelock_set(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    config: TimelockConfig,
    name: [u8; NAME_SIZE],
    desc_link: [u8; DESC_SIZE],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let timelock_set_account_info = next_account_info(account_info_iter)?;
    let signatory_mint_account_info = next_account_info(account_info_iter)?;
    let admin_mint_account_info = next_account_info(account_info_iter)?;
    let voting_mint_account_info = next_account_info(account_info_iter)?;
    let yes_voting_mint_account_info = next_account_info(account_info_iter)?;
    let no_voting_mint_account_info = next_account_info(account_info_iter)?;
    let signatory_validation_account_info = next_account_info(account_info_iter)?;
    let admin_validation_account_info = next_account_info(account_info_iter)?;
    let voting_validation_account_info = next_account_info(account_info_iter)?;
    let destination_admin_account_info = next_account_info(account_info_iter)?;
    let destination_sig_account_info = next_account_info(account_info_iter)?;
    let governance_holding_account_info = next_account_info(account_info_iter)?;
    let governance_mint_account_info = next_account_info(account_info_iter)?;
    let timelock_program_authority_info = next_account_info(account_info_iter)?;
    let timelock_program_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let rent_info = next_account_info(account_info_iter)?;
    let rent = &Rent::from_account_info(rent_info)?;

    let timelock_program: TimelockProgram = assert_initialized(timelock_program_info)?;

    assert_rent_exempt(rent, timelock_set_account_info)?;

    let mut new_timelock_set: TimelockSet = assert_uninitialized(timelock_set_account_info)?;
    new_timelock_set.version = TIMELOCK_SET_VERSION;
    new_timelock_set.config = config;
    new_timelock_set.state.desc_link = desc_link;
    new_timelock_set.state.name = name;
    new_timelock_set.state.total_signing_tokens_minted = 1;

    assert_same_version_as_program(&timelock_program, &new_timelock_set)?;
    assert_token_program_is_correct(&timelock_program, token_program_info)?;
    // now create the mints.

    new_timelock_set.admin_mint = *admin_mint_account_info.key;
    new_timelock_set.voting_mint = *voting_mint_account_info.key;
    new_timelock_set.yes_voting_mint = *yes_voting_mint_account_info.key;
    new_timelock_set.no_voting_mint = *no_voting_mint_account_info.key;
    new_timelock_set.signatory_mint = *signatory_mint_account_info.key;

    if new_timelock_set.config.timelock_type == TimelockType::Governance  {
        let _governance_mint: Mint = assert_initialized(governance_mint_account_info)?;
        let _governance_holding: Account = assert_initialized(governance_holding_account_info)?;
        new_timelock_set.governance_mint = *governance_mint_account_info.key;
        new_timelock_set.governance_holding = *governance_holding_account_info.key;
    }

    new_timelock_set.admin_validation = *admin_validation_account_info.key;
    new_timelock_set.voting_validation = *voting_validation_account_info.key;
    new_timelock_set.signatory_validation = *signatory_validation_account_info.key;

    assert_rent_exempt(rent, admin_mint_account_info)?;
    assert_rent_exempt(rent, voting_mint_account_info)?;
    assert_rent_exempt(rent, yes_voting_mint_account_info)?;
    assert_rent_exempt(rent, no_voting_mint_account_info)?;
    assert_rent_exempt(rent, signatory_mint_account_info)?;
    assert_rent_exempt(rent, governance_holding_account_info)?;
    assert_rent_exempt(rent, admin_validation_account_info)?;
    assert_rent_exempt(rent, signatory_validation_account_info)?;
    assert_rent_exempt(rent, voting_validation_account_info)?;

    let _admin_mint: Mint = assert_initialized(admin_mint_account_info)?;
    let _voting_mint: Mint = assert_initialized(voting_mint_account_info)?;
    let _yes_voting_mint: Mint = assert_initialized(yes_voting_mint_account_info)?;
    let _no_voting_mint: Mint = assert_initialized(no_voting_mint_account_info)?;
    let _signatory_mint: Mint = assert_initialized(signatory_mint_account_info)?;
    let _sig_acct: Account = assert_initialized(destination_sig_account_info)?;
    let _admin_acct: Account = assert_initialized(destination_admin_account_info)?;

    TimelockSet::pack(
        new_timelock_set.clone(),
        &mut timelock_set_account_info.data.borrow_mut(),
    )?;

    let (authority_key, bump_seed) =
        Pubkey::find_program_address(&[timelock_program_info.key.as_ref()], program_id);
    if timelock_program_authority_info.key != &authority_key {
        return Err(TimelockError::InvalidTimelockAuthority.into());
    }
    let authority_signer_seeds = &[timelock_program_info.key.as_ref(), &[bump_seed]];

    spl_token_mint_to(TokenMintToParams {
        mint: admin_mint_account_info.clone(),
        destination: destination_admin_account_info.clone(),
        amount: 1,
        authority: timelock_program_authority_info.clone(),
        authority_signer_seeds: authority_signer_seeds,
        token_program: token_program_info.clone(),
    })?;

    spl_token_mint_to(TokenMintToParams {
        mint: signatory_mint_account_info.clone(),
        destination: destination_sig_account_info.clone(),
        amount: 1,
        authority: timelock_program_authority_info.clone(),
        authority_signer_seeds: authority_signer_seeds,
        token_program: token_program_info.clone(),
    })?;
    Ok(())
}
