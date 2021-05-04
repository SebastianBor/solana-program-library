//! Program state processor
use crate::{
    error::TimelockError,
    state::proposal::Proposal,
    state::proposal_state::ProposalState,
    utils::{
        assert_account_equiv, assert_draft, assert_initialized, assert_is_permissioned,
        assert_token_program_is_correct, spl_token_burn, TokenBurnParams,
    },
    PROGRAM_AUTHORITY_SEED,
};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    program_pack::Pack,
    pubkey::Pubkey,
};

/// Removes a signer
pub fn process_remove_signer(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let remove_signatory_account_info = next_account_info(account_info_iter)?;
    let signatory_mint_info = next_account_info(account_info_iter)?;
    let admin_account_info = next_account_info(account_info_iter)?;
    let admin_validation_account_info = next_account_info(account_info_iter)?;
    let timelock_state_account_info = next_account_info(account_info_iter)?;
    let timelock_set_account_info = next_account_info(account_info_iter)?;
    let transfer_authority_info = next_account_info(account_info_iter)?;
    let timelock_program_authority_info = next_account_info(account_info_iter)?;
    let token_program_account_info = next_account_info(account_info_iter)?;

    let mut timelock_state: ProposalState = assert_initialized(timelock_state_account_info)?;
    let timelock_set: Proposal = assert_initialized(timelock_set_account_info)?;

    assert_account_equiv(timelock_state_account_info, &timelock_set.state)?;
    assert_account_equiv(signatory_mint_info, &timelock_set.signatory_mint)?;
    assert_account_equiv(
        admin_validation_account_info,
        &timelock_set.admin_validation,
    )?;
    assert_token_program_is_correct(&timelock_set, token_program_account_info)?;
    assert_draft(&timelock_state)?;
    assert_is_permissioned(
        program_id,
        admin_account_info,
        admin_validation_account_info,
        timelock_set_account_info,
        token_program_account_info,
        transfer_authority_info,
        timelock_program_authority_info,
    )?;

    let mut seeds = vec![
        PROGRAM_AUTHORITY_SEED,
        timelock_set_account_info.key.as_ref(),
    ];

    let (authority_key, bump_seed) = Pubkey::find_program_address(&seeds[..], program_id);
    if timelock_program_authority_info.key != &authority_key {
        return Err(TimelockError::InvalidTimelockAuthority.into());
    }
    let bump = &[bump_seed];
    seeds.push(bump);
    let authority_signer_seeds = &seeds[..];

    // Remove the token
    spl_token_burn(TokenBurnParams {
        mint: signatory_mint_info.clone(),
        amount: 1,
        authority: transfer_authority_info.clone(),
        authority_signer_seeds,
        token_program: token_program_account_info.clone(),
        source: remove_signatory_account_info.clone(),
    })?;
    timelock_state.total_signing_tokens_minted -= 1;

    ProposalState::pack(
        timelock_state,
        &mut timelock_state_account_info.data.borrow_mut(),
    )?;
    Ok(())
}
