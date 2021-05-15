//! Program state processor

use crate::{
    state::{enums::ProposalState, z_proposal::ProposalOld, z_proposal_state::ProposalStateOld},
    utils::{
        assert_account_equiv, assert_initialized, assert_is_permissioned,
        assert_not_in_voting_or_executing, assert_token_program_is_correct,
    },
};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    program_pack::Pack,
    pubkey::Pubkey,
};

/// Cancel Proposal
pub fn process_cancel_proposal(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let proposal_state_account_info = next_account_info(account_info_iter)?;
    let admin_account_info = next_account_info(account_info_iter)?;
    let admin_validation_account_info = next_account_info(account_info_iter)?;
    let proposal_account_info = next_account_info(account_info_iter)?;
    let transfer_authority_info = next_account_info(account_info_iter)?;
    let proposal_authority_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;

    let mut proposal_state: ProposalStateOld = assert_initialized(proposal_state_account_info)?;
    let proposal: ProposalOld = assert_initialized(proposal_account_info)?;

    assert_account_equiv(admin_validation_account_info, &proposal.admin_validation)?;
    assert_account_equiv(proposal_state_account_info, &proposal.state)?;
    assert_token_program_is_correct(token_program_info)?;
    assert_not_in_voting_or_executing(&proposal_state)?;
    assert_is_permissioned(
        program_id,
        admin_account_info,
        admin_validation_account_info,
        proposal_account_info,
        token_program_info,
        transfer_authority_info,
        proposal_authority_info,
    )?;
    proposal_state.status = ProposalState::Cancelled;
    ProposalStateOld::pack(
        proposal_state,
        &mut proposal_state_account_info.data.borrow_mut(),
    )?;
    Ok(())
}
