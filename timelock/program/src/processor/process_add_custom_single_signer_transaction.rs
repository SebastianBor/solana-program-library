//! Program state processor
use crate::{
    error::TimelockError,
    state::{
        custom_single_signer_timelock_transaction::{
            CustomSingleSignerTimelockTransaction, INSTRUCTION_LIMIT,
        },
        enums::GovernanceAccountType,
        proposal::Proposal,
        timelock_config::Governance,
        timelock_state::{TimelockState, MAX_TRANSACTIONS},
    },
    utils::{
        assert_account_equiv, assert_draft, assert_initialized, assert_is_permissioned,
        assert_token_program_is_correct, assert_uninitialized,
    },
};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    program_pack::Pack,
    pubkey::Pubkey,
};

/// Create a new timelock txn
pub fn process_add_custom_single_signer_transaction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    slot: u64,
    instruction: [u8; INSTRUCTION_LIMIT],
    position: u8,
    instruction_end_index: u16,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let timelock_txn_account_info = next_account_info(account_info_iter)?;
    let timelock_state_account_info = next_account_info(account_info_iter)?;
    let signatory_account_info = next_account_info(account_info_iter)?;
    let signatory_validation_account_info = next_account_info(account_info_iter)?;
    let timelock_set_account_info = next_account_info(account_info_iter)?;
    let timelock_config_account_info = next_account_info(account_info_iter)?;
    let transfer_authority_info = next_account_info(account_info_iter)?;
    let timelock_mint_authority_info = next_account_info(account_info_iter)?;
    let token_program_account_info = next_account_info(account_info_iter)?;

    let mut timelock_state: TimelockState = assert_initialized(timelock_state_account_info)?;
    let timelock_set: Proposal = assert_initialized(timelock_set_account_info)?;
    let timelock_config: Governance = assert_initialized(timelock_config_account_info)?;

    let mut timelock_txn: CustomSingleSignerTimelockTransaction =
        assert_uninitialized(timelock_txn_account_info)?;

    if position as usize >= MAX_TRANSACTIONS {
        return Err(TimelockError::TooHighPositionInTxnArrayError.into());
    }

    if instruction_end_index as usize >= INSTRUCTION_LIMIT as usize {
        return Err(TimelockError::InvalidInstructionEndIndex.into());
    }

    assert_account_equiv(
        signatory_validation_account_info,
        &timelock_set.signatory_validation,
    )?;
    assert_account_equiv(timelock_state_account_info, &timelock_set.state)?;
    assert_draft(&timelock_state)?;
    assert_token_program_is_correct(&timelock_set, token_program_account_info)?;
    assert_is_permissioned(
        program_id,
        signatory_account_info,
        signatory_validation_account_info,
        timelock_set_account_info,
        token_program_account_info,
        transfer_authority_info,
        timelock_mint_authority_info,
    )?;

    if slot < timelock_config.minimum_slot_waiting_period {
        return Err(TimelockError::MustBeAboveMinimumWaitingPeriod.into());
    };

    timelock_txn.account_type = GovernanceAccountType::CustomSingleSignerTransaction;
    timelock_txn.slot = slot;
    timelock_txn.instruction = instruction;
    timelock_txn.instruction_end_index = instruction_end_index;
    timelock_state.timelock_transactions[position as usize] = *timelock_txn_account_info.key;
    timelock_state.number_of_transactions =
        match timelock_state.number_of_transactions.checked_add(1) {
            Some(val) => val,
            None => return Err(TimelockError::NumericalOverflow.into()),
        };

    TimelockState::pack(
        timelock_state,
        &mut timelock_state_account_info.data.borrow_mut(),
    )?;

    CustomSingleSignerTimelockTransaction::pack(
        timelock_txn,
        &mut timelock_txn_account_info.data.borrow_mut(),
    )?;

    Ok(())
}
