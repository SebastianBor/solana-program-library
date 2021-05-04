//! Program state processor

use crate::{
    error::TimelockError,
    state::timelock_program::TimelockProgram,
    state::{enums::TimelockStateStatus, timelock_set::TimelockSet},
    utils::{
        assert_initialized, assert_token_program_is_correct, spl_token_burn, spl_token_transfer,
        TokenBurnParams, TokenTransferParams,
    },
};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};
use spl_token::state::Account;

/// Withdraw voting tokens
pub fn process_withdraw_voting_tokens(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    voting_token_amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let voting_account_info = next_account_info(account_info_iter)?;
    let yes_voting_account_info = next_account_info(account_info_iter)?;
    let no_voting_account_info = next_account_info(account_info_iter)?;
    let destination_governance_account_info = next_account_info(account_info_iter)?;
    let governance_holding_account_info = next_account_info(account_info_iter)?;
    let yes_voting_dump_account_info = next_account_info(account_info_iter)?;
    let no_voting_dump_account_info = next_account_info(account_info_iter)?;
    let voting_mint_account_info = next_account_info(account_info_iter)?;

    let timelock_set_account_info = next_account_info(account_info_iter)?;

    let transfer_authority_info = next_account_info(account_info_iter)?;
    let yes_transfer_authority_info = next_account_info(account_info_iter)?;
    let no_transfer_authority_info = next_account_info(account_info_iter)?;
    let timelock_program_authority_info = next_account_info(account_info_iter)?;
    let timelock_program_account_info = next_account_info(account_info_iter)?;
    let token_program_account_info = next_account_info(account_info_iter)?;

    let timelock_set: TimelockSet = assert_initialized(timelock_set_account_info)?;
    let timelock_program: TimelockProgram = assert_initialized(timelock_program_account_info)?;
    assert_token_program_is_correct(&timelock_program, token_program_account_info)?;
    // Using assert_account_equiv not workable here due to cost of stack size on this method.
    if voting_mint_account_info.key != &timelock_set.voting_mint {
        return Err(TimelockError::AccountsShouldMatch.into());
    }
    if yes_voting_dump_account_info.key != &timelock_set.yes_voting_dump {
        return Err(TimelockError::AccountsShouldMatch.into());
    }
    if no_voting_dump_account_info.key != &timelock_set.no_voting_dump {
        return Err(TimelockError::AccountsShouldMatch.into());
    }
    if governance_holding_account_info.key != &timelock_set.governance_holding {
        return Err(TimelockError::AccountsShouldMatch.into());
    }

    if voting_token_amount < 0 as u64 {
        return Err(TimelockError::TokenAmountBelowZero.into());
    }

    let voting_account: Account = assert_initialized(voting_account_info)?;
    let yes_voting_account: Account = assert_initialized(yes_voting_account_info)?;
    let no_voting_account: Account = assert_initialized(no_voting_account_info)?;

    let (authority_key, bump_seed) =
        Pubkey::find_program_address(&[timelock_program_account_info.key.as_ref()], program_id);
    if timelock_program_authority_info.key != &authority_key {
        return Err(TimelockError::InvalidTimelockAuthority.into());
    }
    let authority_signer_seeds = &[timelock_program_account_info.key.as_ref(), &[bump_seed]];

    // prefer voting account first, then yes, then no. Invariants we know are
    // voting_token_amount <= voting + yes + no
    // voting_token_amount <= voting
    // voting_token_amount <= yes
    // voting_token_amount <= no
    // because at best they dumped 100 in and that 100 is mixed between all 3 or all in one.

    let total_possible: u64;

    if timelock_set.state.status == TimelockStateStatus::Voting {
        total_possible = voting_account.amount
    } else {
        total_possible =
            voting_account.amount + yes_voting_account.amount + no_voting_account.amount;
    };

    let mut voting_fuel_tank = voting_token_amount;
    if voting_token_amount > total_possible {
        return Err(TimelockError::TokenAmountAboveGivenAmount.into());
    }

    if voting_account.amount > 0 {
        let amount_to_burn: u64;
        if voting_account.amount < voting_fuel_tank {
            amount_to_burn = voting_account.amount;
            voting_fuel_tank -= amount_to_burn;
        } else {
            amount_to_burn = voting_fuel_tank;
            voting_fuel_tank = 0;
        }
        if amount_to_burn > 0 {
            spl_token_burn(TokenBurnParams {
                mint: voting_mint_account_info.clone(),
                amount: amount_to_burn,
                authority: transfer_authority_info.clone(),
                authority_signer_seeds: authority_signer_seeds,
                token_program: token_program_account_info.clone(),
                source: voting_account_info.clone(),
            })?;
        }
    }

    if timelock_set.state.status != TimelockStateStatus::Voting {
        if yes_voting_account.amount > 0 {
            let amount_to_transfer: u64;
            if yes_voting_account.amount < voting_fuel_tank {
                amount_to_transfer = yes_voting_account.amount;
                voting_fuel_tank -= amount_to_transfer;
            } else {
                amount_to_transfer = voting_fuel_tank;
                voting_fuel_tank = 0;
            }

            if amount_to_transfer > 0 {
                spl_token_transfer(TokenTransferParams {
                    source: yes_voting_account_info.clone(),
                    destination: yes_voting_dump_account_info.clone(),
                    amount: amount_to_transfer,
                    authority: yes_transfer_authority_info.clone(),
                    authority_signer_seeds: authority_signer_seeds,
                    token_program: token_program_account_info.clone(),
                })?;
            }
        }

        if no_voting_account.amount > 0 && voting_fuel_tank > 0 {
            // whatever is left, no account gets by default
            spl_token_transfer(TokenTransferParams {
                source: no_voting_account_info.clone(),
                destination: no_voting_dump_account_info.clone(),
                amount: voting_fuel_tank,
                authority: no_transfer_authority_info.clone(),
                authority_signer_seeds: authority_signer_seeds,
                token_program: token_program_account_info.clone(),
            })?;
        }
    }

    spl_token_transfer(TokenTransferParams {
        source: governance_holding_account_info.clone(),
        destination: destination_governance_account_info.clone(),
        amount: voting_token_amount,
        authority: timelock_program_authority_info.clone(),
        authority_signer_seeds: authority_signer_seeds,
        token_program: token_program_account_info.clone(),
    })?;

    Ok(())
}
