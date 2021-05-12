//! General purpose token utility functions

use arrayref::array_ref;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
};

use crate::error::GovernanceError;

pub fn create_spl_token_account<'a>(
    payer_info: &AccountInfo<'a>,
    token_account_info: &AccountInfo<'a>,
    token_mint_info: &AccountInfo<'a>,
    token_account_owner_info: &AccountInfo<'a>,
    system_info: &AccountInfo<'a>,
    spl_token_info: &AccountInfo<'a>,
    rent_sysvar_info: &AccountInfo<'a>,
) -> Result<(), ProgramError> {
    let create_account_instruction = system_instruction::create_account(
        payer_info.key,
        token_account_info.key,
        1.max(Rent::default().minimum_balance(spl_token::state::Account::get_packed_len())),
        spl_token::state::Account::get_packed_len() as u64,
        &spl_token::id(),
    );

    invoke(
        &create_account_instruction,
        &[
            payer_info.clone(),
            token_account_info.clone(),
            system_info.clone(),
        ],
    )?;

    let initialize_account_instruction = spl_token::instruction::initialize_account(
        &spl_token::id(),
        token_account_info.key,
        token_mint_info.key,
        token_account_owner_info.key,
    )?;

    invoke(
        &initialize_account_instruction,
        &[
            payer_info.clone(),
            token_account_info.clone(),
            token_account_owner_info.clone(),
            token_mint_info.clone(),
            spl_token_info.clone(),
            rent_sysvar_info.clone(),
        ],
    )?;

    Ok(())
}

pub fn create_spl_token_account_signed<'a>(
    payer_info: &AccountInfo<'a>,
    token_account_info: &AccountInfo<'a>,
    token_account_address_seeds: Vec<&[u8]>,
    token_mint_info: &AccountInfo<'a>,
    token_account_owner_info: &AccountInfo<'a>,
    program_id: &Pubkey,
    system_info: &AccountInfo<'a>,
    spl_token_info: &AccountInfo<'a>,
    rent_sysvar_info: &AccountInfo<'a>,
) -> Result<(), ProgramError> {
    let create_account_instruction = system_instruction::create_account(
        payer_info.key,
        token_account_info.key,
        1.max(Rent::default().minimum_balance(spl_token::state::Account::get_packed_len())),
        spl_token::state::Account::get_packed_len() as u64,
        &spl_token::id(),
    );

    let (account_address, bump_seed) =
        Pubkey::find_program_address(&token_account_address_seeds[..], program_id);

    if account_address != *token_account_info.key {
        msg!(
                "Create SPL Token Account with Program Derived Address: {:?} was requested while Address: {:?} was expected",
                token_account_info.key,
                account_address
            );
        return Err(ProgramError::InvalidSeeds);
    }

    let mut signers_seeds = token_account_address_seeds.to_vec();
    let bump = &[bump_seed];
    signers_seeds.push(bump);

    invoke_signed(
        &create_account_instruction,
        &[
            payer_info.clone(),
            token_account_info.clone(),
            system_info.clone(),
        ],
        &[&signers_seeds[..]],
    )?;

    let initialize_account_instruction = spl_token::instruction::initialize_account(
        &spl_token::id(),
        token_account_info.key,
        token_mint_info.key,
        token_account_owner_info.key,
    )?;

    invoke(
        &initialize_account_instruction,
        &[
            payer_info.clone(),
            token_account_info.clone(),
            token_account_owner_info.clone(),
            token_mint_info.clone(),
            spl_token_info.clone(),
            rent_sysvar_info.clone(),
        ],
    )?;

    Ok(())
}

pub fn transfer_spl_tokens<'a>(
    source_info: &AccountInfo<'a>,
    destination_info: &AccountInfo<'a>,
    authority_info: &AccountInfo<'a>,
    amount: u64,
    spl_token_info: &AccountInfo<'a>,
) -> ProgramResult {
    let transfer_instruction = spl_token::instruction::transfer(
        &spl_token::id(),
        source_info.key,
        destination_info.key,
        authority_info.key,
        &[],
        amount,
    )
    .unwrap();

    invoke(
        &transfer_instruction,
        &[
            spl_token_info.clone(),
            authority_info.clone(),
            source_info.clone(),
            destination_info.clone(),
        ],
    )?;

    Ok(())
}

pub fn transfer_spl_tokens_signed<'a>(
    source_info: &AccountInfo<'a>,
    destination_info: &AccountInfo<'a>,
    authority_info: &AccountInfo<'a>,
    authority_seeds: Vec<&[u8]>,
    source_owner: &Pubkey,
    amount: u64,
    spl_token_info: &AccountInfo<'a>,
) -> ProgramResult {
    let (authority_address, bump_seed) =
        Pubkey::find_program_address(&authority_seeds[..], source_owner);

    if authority_address != *authority_info.key {
        msg!(
                "Transfer SPL Token with Authority Address: {:?} was requested while Address: {:?} was expected",
                authority_info.key,
                authority_address
            );
        return Err(ProgramError::InvalidSeeds);
    }

    let transfer_instruction = spl_token::instruction::transfer(
        &spl_token::id(),
        source_info.key,
        destination_info.key,
        authority_info.key,
        &[],
        amount,
    )
    .unwrap();

    let mut signers_seeds = authority_seeds.to_vec();
    let bump = &[bump_seed];
    signers_seeds.push(bump);

    invoke_signed(
        &transfer_instruction,
        &[
            spl_token_info.clone(),
            authority_info.clone(),
            source_info.clone(),
            destination_info.clone(),
        ],
        &[&signers_seeds[..]],
    )?;

    Ok(())
}

/// Computationally cheap method to get amount from a token account. It reads amount without deserializing full account data
pub fn get_amount_from_token_account(
    token_account_info: &AccountInfo,
) -> Result<u64, ProgramError> {
    if token_account_info.owner != &spl_token::id() {
        return Err(GovernanceError::InvalidTokenAccountOwnerError.into());
    }

    // TokeAccount layout:   mint(32), owner(32), amount(8)
    let data = token_account_info.try_borrow_data()?;
    let amount = array_ref![data, 64, 8];
    Ok(u64::from_le_bytes(*amount))
}

/// Computationally cheap method to get mint from a token account. It reads mint without deserializing full account data
pub fn get_mint_from_token_account(
    token_account_info: &AccountInfo,
) -> Result<Pubkey, ProgramError> {
    if token_account_info.owner != &spl_token::id() {
        return Err(GovernanceError::InvalidTokenAccountOwnerError.into());
    }

    // TokeAccount layout:   mint(32), owner(32), amount(8)
    let data = token_account_info.try_borrow_data().unwrap();
    let mint_data = array_ref![data, 0, 32];
    Ok(Pubkey::new_from_array(*mint_data))
}
