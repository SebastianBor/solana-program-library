//! Program state processor
use crate::{error::TimelockError, state::timelock_program::TimelockProgram, state::timelock_set::TimelockSet, utils::{assert_account_equiv, assert_draft, assert_initialized, assert_is_permissioned, assert_same_version_as_program}};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    program_pack::Pack,
    pubkey::Pubkey,
};

/// Removes a txn from a transaction set
pub fn process_remove_transaction(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let timelock_set_account_info = next_account_info(account_info_iter)?;
    let timelock_txn_account_info = next_account_info(account_info_iter)?;
    let signatory_account_info = next_account_info(account_info_iter)?;
    let signatory_validation_account_info = next_account_info(account_info_iter)?;
    let transfer_authority_info = next_account_info(account_info_iter)?;
    let timelock_authority_account_info = next_account_info(account_info_iter)?;
    let timelock_program_account_info = next_account_info(account_info_iter)?;
    let token_program_account_info = next_account_info(account_info_iter)?;

    let mut timelock_set: TimelockSet = assert_initialized(timelock_set_account_info)?;
    let timelock_program: TimelockProgram = assert_initialized(timelock_program_account_info)?;
    assert_account_equiv(signatory_validation_account_info, &timelock_set.signatory_validation)?;
    assert_same_version_as_program(&timelock_program, &timelock_set)?;
    assert_draft(&timelock_set)?;
    assert_is_permissioned(
        program_id,
        signatory_account_info,
        signatory_validation_account_info,
        timelock_program_account_info,
        token_program_account_info,
        transfer_authority_info,
        timelock_authority_account_info,
    )?;

    let mut found: bool = false;
    for n in 0..timelock_set.state.timelock_transactions.len() {
        if timelock_set.state.timelock_transactions[n].to_bytes()
            == timelock_txn_account_info.key.to_bytes()
        {
            let zeros: [u8; 32] = [0; 32];
            timelock_set.state.timelock_transactions[n] = Pubkey::new_from_array(zeros);
            found = true;
            break;
        }
    }

    if !found {
        return Err(TimelockError::TimelockTransactionNotFoundError.into());
    }

    TimelockSet::pack(
        timelock_set.clone(),
        &mut timelock_set_account_info.data.borrow_mut(),
    )?;

    Ok(())
}
