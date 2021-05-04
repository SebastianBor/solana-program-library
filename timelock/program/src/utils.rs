use crate::{
    error::TimelockError,
    state::{enums::TimelockStateStatus, timelock_set::TimelockSet, timelock_state::TimelockState},
};
use arrayref::{array_ref, array_refs};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::Instruction,
    program::invoke_signed,
    program_error::ProgramError,
    program_option::COption,
    program_pack::{IsInitialized, Pack},
    pubkey::Pubkey,
    system_instruction::create_account,
    sysvar::rent::Rent,
};
use spl_token::state::Account;

/* TODO come back to this conundrum later..

pub fn get_authority_signer_seeds<'a>(
    timelock_program_account_info: &'a AccountInfo<'a>,
    timelock_program_authority_info: &'a AccountInfo<'a>,
    program_id: &'a Pubkey,
) -> Result<&'a [&'a [u8]; 2], ProgramError> {
    let (authority_key, bump_seed) =
        Pubkey::find_program_address(&[timelock_program_account_info.key.as_ref()], program_id);
    if timelock_program_authority_info.key != &authority_key {
        return Err(TimelockError::InvalidTimelockAuthority.into());
    }
    let authority_signer_seeds = &[timelock_program_account_info.key.as_ref(), &[bump_seed]];
    Ok(&*authority_signer_seeds)
}*/

/// Attempts to transfer the token to the timelock set's validation account and back to the person again.
/// Can only be done if done in a transaction that has authority to do so. Serves as a check
/// That the person is who they say they are!
pub fn assert_is_permissioned<'a>(
    program_id: &Pubkey,
    perm_account_info: &AccountInfo<'a>,
    perm_validation_account_info: &AccountInfo<'a>,
    timelock_set_info: &AccountInfo<'a>,
    token_program_info: &AccountInfo<'a>,
    transfer_authority_info: &AccountInfo<'a>,
    timelock_authority_info: &AccountInfo<'a>,
) -> ProgramResult {
    let _perm_account: Account = assert_initialized(perm_account_info)?;
    let _perm_validation: Account = assert_initialized(perm_validation_account_info)?;
    let (authority_key, bump_seed) =
        Pubkey::find_program_address(&[timelock_set_info.key.as_ref()], program_id);
    if timelock_authority_info.key != &authority_key {
        return Err(TimelockError::InvalidTimelockAuthority.into());
    }
    let authority_signer_seeds = &[timelock_set_info.key.as_ref(), &[bump_seed]];
    // If both accounts arent correct mint type, it explodes
    // If token amount is <1, it explodes. Perfect check.
    // If authority isnt right, it explodes.
    spl_token_transfer(TokenTransferParams {
        source: perm_account_info.clone(),
        destination: perm_validation_account_info.clone(),
        amount: 1,
        authority: transfer_authority_info.clone(),
        authority_signer_seeds,
        token_program: token_program_info.clone(),
    })?;
    // Now give it back
    spl_token_transfer(TokenTransferParams {
        source: perm_validation_account_info.clone(),
        destination: perm_account_info.clone(),
        amount: 1,
        authority: timelock_authority_info.clone(),
        authority_signer_seeds,
        token_program: token_program_info.clone(),
    })?;
    Ok(())
}

/// Asserts a timelock set is in a state that can be edited - if its voting or executing, cant touch it.
pub fn assert_not_in_voting_or_executing(timelock_state: &TimelockState) -> ProgramResult {
    if timelock_state.status == TimelockStateStatus::Voting
        || timelock_state.status == TimelockStateStatus::Executing
    {
        return Err(TimelockError::InvalidTimelockSetStateError.into());
    }
    Ok(())
}

/// Asserts a timelock set is in executing state.
pub fn assert_executing(timelock_state: &TimelockState) -> ProgramResult {
    if timelock_state.status != TimelockStateStatus::Executing {
        return Err(TimelockError::InvalidTimelockSetStateError.into());
    }
    Ok(())
}

/// Asserts a timelock set is in voting state.
pub fn assert_voting(timelock_state: &TimelockState) -> ProgramResult {
    if timelock_state.status != TimelockStateStatus::Voting {
        return Err(TimelockError::InvalidTimelockSetStateError.into());
    }
    Ok(())
}

/// Asserts a timelock set is in draft state.
pub fn assert_draft(timelock_state: &TimelockState) -> ProgramResult {
    if timelock_state.status != TimelockStateStatus::Draft {
        return Err(TimelockError::InvalidTimelockSetStateError.into());
    }
    Ok(())
}

/// Asserts the proper mint key is being used.
pub fn assert_proper_signatory_mint(
    timelock_set: &TimelockSet,
    signatory_mint_account_info: &AccountInfo,
) -> ProgramResult {
    if timelock_set.signatory_mint != *signatory_mint_account_info.key {
        return Err(TimelockError::InvalidSignatoryMintError.into());
    }
    Ok(())
}

/// Asserts token_program is correct program
pub fn assert_token_program_is_correct(
    timelock_program: &TimelockSet,
    token_program_info: &AccountInfo,
) -> ProgramResult {
    if &timelock_program.token_program_id != token_program_info.key {
        return Err(TimelockError::InvalidTokenProgram.into());
    };

    Ok(())
}

/// asserts timelock txn is in timelock set
pub fn assert_txn_in_state(
    timelock_state: &TimelockState,
    timelock_txn_account_info: &AccountInfo,
) -> ProgramResult {
    let mut found: bool = false;
    for n in 0..timelock_state.timelock_transactions.len() {
        if timelock_state.timelock_transactions[n].to_bytes()
            == timelock_txn_account_info.key.to_bytes()
        {
            found = true;
            break;
        }
    }

    if !found {
        return Err(TimelockError::TimelockTransactionNotFoundError.into());
    }

    Ok(())
}

/// asserts that two accounts are equivalent
pub fn assert_account_equiv(acct: &AccountInfo, key: &Pubkey) -> ProgramResult {
    if acct.key != key {
        return Err(TimelockError::AccountsShouldMatch.into());
    }

    Ok(())
}

/// Cheaper Assertion the account has a matching mint - if you don't plan to use Mint for anything else
pub fn assert_mint_matching(acct: &AccountInfo, mint: &AccountInfo) -> ProgramResult {
    let mint_key: Pubkey = get_mint_from_account(acct)?;
    if &mint_key != mint.key {
        return Err(TimelockError::MintsShouldMatch.into());
    }

    Ok(())
}

/// Cheaper Assertion the account has a matching mint decimals - if you don't plan to use Mint for anything else
pub fn assert_mint_decimals(mint: &AccountInfo, mint_decimals: u8) -> ProgramResult {
    if get_mint_decimals(mint).unwrap() != mint_decimals {
        return Err(TimelockError::MintsDecimalsShouldMatch.into());
    }

    Ok(())
}

/// Cheaper Assertion the account has a matching mint_authority- if you don't plan to use Mint for anything else
pub fn assert_mint_authority(mint: &AccountInfo, mint_authority: &Pubkey) -> ProgramResult {
    if get_mint_authority(mint).unwrap() != *mint_authority {
        return Err(TimelockError::InvalidMintAuthorityError.into());
    }
    Ok(())
}

/// assert rent exempt
pub fn assert_rent_exempt(rent: &Rent, account_info: &AccountInfo) -> ProgramResult {
    if !rent.is_exempt(account_info.lamports(), account_info.data_len()) {
        Err(TimelockError::NotRentExempt.into())
    } else {
        Ok(())
    }
}
/// assert ununitialized account
pub fn assert_uninitialized<T: Pack + IsInitialized>(
    account_info: &AccountInfo,
) -> Result<T, ProgramError> {
    let account: T = T::unpack_unchecked(&account_info.data.borrow())?;
    if account.is_initialized() {
        Err(TimelockError::AlreadyInitialized.into())
    } else {
        Ok(account)
    }
}

/// cheap assertion of mint is_initialized without unpacking whole object
pub fn assert_mint_initialized(account_info: &AccountInfo) -> Result<(), ProgramError> {
    // In token program, 36, 8, 1, 1 is the layout, where the last 1 is initialized bit.
    // Not my favorite hack, but necessary to avoid stack size limitations caused by serializing entire Mint
    // to get at initialization check
    let index: usize = 36 + 8 + 1 + 1 - 1;
    if account_info.try_borrow_data().unwrap()[index] == 0 {
        return Err(TimelockError::Uninitialized.into());
    }
    Ok(())
}

/// cheap method to just get supply off a mint without unpacking whole object
pub fn get_mint_supply(account_info: &AccountInfo) -> Result<u64, ProgramError> {
    // In token program, 36, 8, 1, 1 is the layout, where the first 8 is supply u64.
    // so we start at 36.
    let data = account_info.try_borrow_data().unwrap();
    let bytes = array_ref![data, 36, 8];

    Ok(u64::from_le_bytes(*bytes))
}

/// cheap method to just get supply off a mint without unpacking whole object
pub fn get_mint_authority(account_info: &AccountInfo) -> Result<Pubkey, ProgramError> {
    // In token program, 36, 8, 1, 1 is the layout, where the first 36 is mint_authority
    // so we start at 0.
    let data = account_info.try_borrow_data().unwrap();
    let authority_bytes = array_ref![data, 0, 36];

    let authority = unpack_coption_key(&authority_bytes)?;

    match authority {
        COption::Some(pk) => Ok(pk),
        COption::None => Err(TimelockError::MintAuthorityUnpackError.into()),
    }
}

/// cheap method to just get decimals off a mint without unpacking whole object
pub fn get_mint_decimals(account_info: &AccountInfo) -> Result<u8, ProgramError> {
    // In token program, 36, 8, 1, 1 is the Mint layout, where the first 1 is decimals u8.
    // so we start at 44.
    let data = account_info.try_borrow_data().unwrap();
    let bytes = array_ref![data, 44, 1];

    Ok(bytes[0])
}

/// Cheap method to just grab mint Pubkey off token account, instead of deserializing entire thing
pub fn get_mint_from_account(account_info: &AccountInfo) -> Result<Pubkey, ProgramError> {
    // Accounts have mint in first 32 bits.
    let data = account_info.try_borrow_data().unwrap();
    let key_data = array_ref![data, 0, 32];
    Ok(Pubkey::new_from_array(*key_data))
}

/// assert initialized account
pub fn assert_initialized<T: Pack + IsInitialized>(
    account_info: &AccountInfo,
) -> Result<T, ProgramError> {
    let account: T = T::unpack_unchecked(&account_info.data.borrow())?;
    if !account.is_initialized() {
        Err(TimelockError::Uninitialized.into())
    } else {
        Ok(account)
    }
}

/// Issue a spl_token `Transfer` instruction.
#[inline(always)]
pub fn spl_token_transfer(params: TokenTransferParams<'_, '_>) -> ProgramResult {
    let TokenTransferParams {
        source,
        destination,
        authority,
        token_program,
        amount,
        authority_signer_seeds,
    } = params;
    let result = invoke_signed(
        &spl_token::instruction::transfer(
            token_program.key,
            source.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?,
        &[source, destination, authority, token_program],
        &[authority_signer_seeds],
    );
    result.map_err(|_| TimelockError::TokenTransferFailed.into())
}

/// Issue a spl_token `MintTo` instruction.
pub fn spl_token_mint_to(params: TokenMintToParams<'_, '_>) -> ProgramResult {
    let TokenMintToParams {
        mint,
        destination,
        authority,
        token_program,
        amount,
        authority_signer_seeds,
    } = params;
    let result = invoke_signed(
        &spl_token::instruction::mint_to(
            token_program.key,
            mint.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?,
        &[mint, destination, authority, token_program],
        &[authority_signer_seeds],
    );
    result.map_err(|_| TimelockError::TokenMintToFailed.into())
}

/// Issue a spl_token `Burn` instruction.
#[inline(always)]
pub fn spl_token_burn(params: TokenBurnParams<'_, '_>) -> ProgramResult {
    let TokenBurnParams {
        mint,
        source,
        authority,
        token_program,
        amount,
        authority_signer_seeds,
    } = params;
    let result = invoke_signed(
        &spl_token::instruction::burn(
            token_program.key,
            source.key,
            mint.key,
            authority.key,
            &[],
            amount,
        )?,
        &[source, mint, authority, token_program],
        &[authority_signer_seeds],
    );
    result.map_err(|_| TimelockError::TokenBurnFailed.into())
}

/// Issue a spl_token `Burn` instruction.
#[inline(always)]
pub fn execute(params: ExecuteParams<'_, '_>) -> ProgramResult {
    let ExecuteParams {
        instruction,
        authority_signer_seeds,
        account_infos,
    } = params;

    let result = invoke_signed(
        &instruction,
        &account_infos.as_slice(),
        &[authority_signer_seeds],
    );
    result.map_err(|_| TimelockError::ExecutionFailed.into())
}

/// Unpacks COption from a slice, taken from token program
fn unpack_coption_key(src: &[u8; 36]) -> Result<COption<Pubkey>, ProgramError> {
    let (tag, body) = array_refs![src, 4, 32];
    match *tag {
        [0, 0, 0, 0] => Ok(COption::None),
        [1, 0, 0, 0] => Ok(COption::Some(Pubkey::new_from_array(*body))),
        _ => Err(ProgramError::InvalidAccountData),
    }
}

/// Create account from scratch, stolen from Wormhole, slightly altered for my purposes
/// https://github.com/bartosz-lipinski/wormhole/blob/8478735ea7525043635524a62db2751e59d2bc38/solana/bridge/src/processor.rs#L1335
#[inline(always)]
pub fn create_account_raw<T: Pack>(
    accounts: &[AccountInfo],
    new_account: &Pubkey,
    payer: &Pubkey,
    owner: &Pubkey,
    seeds: &[&[u8]],
) -> Result<(), ProgramError> {
    let size = T::LEN;
    let ix = create_account(
        payer,
        new_account,
        Rent::default().minimum_balance(size as usize),
        size as u64,
        owner,
    );
    invoke_signed(&ix, accounts, &[seeds])
}

///TokenTransferParams
pub struct TokenTransferParams<'a: 'b, 'b> {
    /// source
    pub source: AccountInfo<'a>,
    /// destination
    pub destination: AccountInfo<'a>,
    /// amount
    pub amount: u64,
    /// authority
    pub authority: AccountInfo<'a>,
    /// authority_signer_seeds
    pub authority_signer_seeds: &'b [&'b [u8]],
    /// token_program
    pub token_program: AccountInfo<'a>,
}
/// TokenMintToParams
pub struct TokenMintToParams<'a: 'b, 'b> {
    /// mint
    pub mint: AccountInfo<'a>,
    /// destination
    pub destination: AccountInfo<'a>,
    /// amount
    pub amount: u64,
    /// authority
    pub authority: AccountInfo<'a>,
    /// authority_signer_seeds
    pub authority_signer_seeds: &'b [&'b [u8]],
    /// token_program
    pub token_program: AccountInfo<'a>,
}
/// TokenBurnParams
pub struct TokenBurnParams<'a: 'b, 'b> {
    /// mint
    pub mint: AccountInfo<'a>,
    /// source
    pub source: AccountInfo<'a>,
    /// amount
    pub amount: u64,
    /// authority
    pub authority: AccountInfo<'a>,
    /// authority_signer_seeds
    pub authority_signer_seeds: &'b [&'b [u8]],
    /// token_program
    pub token_program: AccountInfo<'a>,
}

/// ExecuteParams
pub struct ExecuteParams<'a: 'b, 'b> {
    /// Instruction
    pub instruction: Instruction,
    /// authority_signer_seeds
    pub authority_signer_seeds: &'b [&'b [u8]],
    /// Account infos
    pub account_infos: Vec<AccountInfo<'a>>,
}

#[cfg(test)]
mod test {
    use super::*;

    use solana_program::{
        account_info::AccountInfo, clock::Epoch, program_option::COption, pubkey::Pubkey,
    };

    use spl_token::state::Mint;

    #[test]
    pub fn test_assert_mint_decimals() {
        let decimals = 5;

        let mint = Mint {
            mint_authority: COption::None,
            supply: 100,
            decimals,
            is_initialized: true,
            freeze_authority: COption::None,
        };

        let mut data = vec![0; Mint::get_packed_len()];
        Mint::pack(mint, &mut data).unwrap();

        let mut lamports = 0;

        let program_id = Pubkey::new_unique();
        let owner_key = Pubkey::new_unique();
        let mint_account_info = AccountInfo::new(
            &owner_key,
            false,
            false,
            &mut lamports,
            &mut data,
            &program_id,
            false,
            Epoch::default(),
        );

        assert_eq!(assert_mint_decimals(&mint_account_info, decimals), Ok(()));
    }

    #[test]
    pub fn test_assert_mint_authority() {
        let mint_authority = Pubkey::new_unique();

        let mint = Mint {
            mint_authority: COption::Some(mint_authority),
            supply: 100,
            decimals: 5,
            is_initialized: true,
            freeze_authority: COption::None,
        };

        let mut data = vec![0; Mint::get_packed_len()];
        Mint::pack(mint, &mut data).unwrap();

        let mut lamports = 0;

        let program_id = Pubkey::new_unique();
        let owner_key = Pubkey::new_unique();
        let mint_account_info = AccountInfo::new(
            &owner_key,
            false,
            false,
            &mut lamports,
            &mut data,
            &program_id,
            false,
            Epoch::default(),
        );

        assert_eq!(
            assert_mint_authority(&mint_account_info, &mint_authority),
            Ok(())
        );
    }
}
