use super::*;
use crate::{
    error::LendingError,
    math::{Decimal, Rate, TryDiv, TryMul, TrySub},
};
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    entrypoint::ProgramResult,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
};
use std::convert::TryInto;

/// Borrow obligation state
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Obligation {
    /// Version of the obligation
    pub version: u8,
    /// Amount of collateral tokens deposited for this obligation
    pub deposited_collateral_tokens: u64,
    /// Reserve which collateral tokens were deposited into
    pub collateral_reserve: Pubkey,
    /// Borrow rate used for calculating interest.
    pub cumulative_borrow_rate_wads: Decimal,
    /// Amount of tokens borrowed for this obligation plus interest
    pub borrowed_liquidity_wads: Decimal,
    /// Reserve which tokens were borrowed from
    pub borrow_reserve: Pubkey,
    /// Mint address of the tokens for this obligation
    pub token_mint: Pubkey,
}

impl Obligation {
    /// Create new obligation
    pub fn new(params: NewObligationParams) -> Self {
        let NewObligationParams {
            collateral_reserve,
            borrow_reserve,
            token_mint,
            cumulative_borrow_rate_wads,
        } = params;

        Self {
            version: PROGRAM_VERSION,
            deposited_collateral_tokens: 0,
            collateral_reserve,
            cumulative_borrow_rate_wads,
            borrowed_liquidity_wads: Decimal::zero(),
            borrow_reserve,
            token_mint,
        }
    }

    /// Maximum amount of loan that can be closed out by a liquidator due
    /// to the remaining balance being too small to be liquidated normally.
    pub fn max_closeable_amount(&self) -> Result<u64, ProgramError> {
        if self.borrowed_liquidity_wads < Decimal::from(CLOSEABLE_AMOUNT) {
            self.borrowed_liquidity_wads.try_ceil_u64()
        } else {
            Ok(0)
        }
    }

    /// Maximum amount of loan that can be repaid by liquidators
    pub fn max_liquidation_amount(&self) -> Result<u64, ProgramError> {
        self.borrowed_liquidity_wads
            .try_mul(Rate::from_percent(LIQUIDATION_CLOSE_FACTOR))?
            .try_floor_u64()
    }

    /// Ratio of loan balance to collateral value
    pub fn loan_to_value(
        &self,
        collateral_exchange_rate: CollateralExchangeRate,
        borrow_token_price: Decimal,
    ) -> Result<Decimal, ProgramError> {
        let loan = self.borrowed_liquidity_wads;
        let collateral_value = collateral_exchange_rate
            .decimal_collateral_to_liquidity(self.deposited_collateral_tokens.into())?
            .try_div(borrow_token_price)?;
        loan.try_div(collateral_value)
    }

    /// Amount of obligation tokens for given collateral
    pub fn collateral_to_obligation_token_amount(
        &self,
        collateral_amount: u64,
        obligation_token_supply: u64,
    ) -> Result<u64, ProgramError> {
        let withdraw_pct =
            Decimal::from(collateral_amount).try_div(self.deposited_collateral_tokens)?;
        let token_amount: Decimal = withdraw_pct.try_mul(obligation_token_supply)?;
        token_amount.try_floor_u64()
    }

    /// Accrue interest
    pub fn accrue_interest(&mut self, cumulative_borrow_rate: Decimal) -> ProgramResult {
        if cumulative_borrow_rate < self.cumulative_borrow_rate_wads {
            return Err(LendingError::NegativeInterestRate.into());
        }

        let compounded_interest_rate: Rate = cumulative_borrow_rate
            .try_div(self.cumulative_borrow_rate_wads)?
            .try_into()?;

        self.borrowed_liquidity_wads = self
            .borrowed_liquidity_wads
            .try_mul(compounded_interest_rate)?;

        self.cumulative_borrow_rate_wads = cumulative_borrow_rate;

        Ok(())
    }

    /// Liquidate part of obligation
    pub fn liquidate(&mut self, repay_amount: Decimal, withdraw_amount: u64) -> ProgramResult {
        self.borrowed_liquidity_wads = self.borrowed_liquidity_wads.try_sub(repay_amount)?;
        self.deposited_collateral_tokens = self
            .deposited_collateral_tokens
            .checked_sub(withdraw_amount)
            .ok_or(LendingError::MathOverflow)?;
        Ok(())
    }

    /// Repay borrowed tokens
    pub fn repay(
        &mut self,
        liquidity_amount: u64,
        obligation_token_supply: u64,
    ) -> Result<RepayResult, ProgramError> {
        let decimal_repay_amount =
            Decimal::from(liquidity_amount).min(self.borrowed_liquidity_wads);
        let integer_repay_amount = decimal_repay_amount.try_ceil_u64()?;
        if integer_repay_amount == 0 {
            return Err(LendingError::ObligationEmpty.into());
        }

        let repay_pct: Decimal = decimal_repay_amount.try_div(self.borrowed_liquidity_wads)?;
        let collateral_withdraw_amount = {
            let withdraw_amount: Decimal = repay_pct.try_mul(self.deposited_collateral_tokens)?;
            withdraw_amount.try_floor_u64()?
        };

        let obligation_token_amount = self.collateral_to_obligation_token_amount(
            collateral_withdraw_amount,
            obligation_token_supply,
        )?;

        self.liquidate(decimal_repay_amount, collateral_withdraw_amount)?;

        Ok(RepayResult {
            collateral_withdraw_amount,
            obligation_token_amount,
            decimal_repay_amount,
            integer_repay_amount,
        })
    }
}

/// Obligation repay result
pub struct RepayResult {
    /// Amount of collateral to withdraw
    pub collateral_withdraw_amount: u64,
    /// Amount of obligation tokens to burn
    pub obligation_token_amount: u64,
    /// Amount that will be repaid as precise decimal
    pub decimal_repay_amount: Decimal,
    /// Amount that will be repaid as u64
    pub integer_repay_amount: u64,
}

/// Create new obligation
pub struct NewObligationParams {
    /// Collateral reserve address
    pub collateral_reserve: Pubkey,
    /// Borrow reserve address
    pub borrow_reserve: Pubkey,
    /// Obligation token mint address
    pub token_mint: Pubkey,
    /// Borrow rate used for calculating interest.
    pub cumulative_borrow_rate_wads: Decimal,
}

impl Sealed for Obligation {}
impl IsInitialized for Obligation {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_VERSION
    }
}

const OBLIGATION_LEN: usize = 265;
impl Pack for Obligation {
    const LEN: usize = 265;

    /// Unpacks a byte buffer into a [ObligationInfo](struct.ObligationInfo.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, OBLIGATION_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            deposited_collateral_tokens,
            collateral_supply,
            cumulative_borrow_rate,
            borrowed_liquidity_wads,
            borrow_reserve,
            token_mint,
            _padding,
        ) = array_refs![input, 1, 8, 32, 16, 16, 32, 32, 128];
        Ok(Self {
            version: u8::from_le_bytes(*version),
            deposited_collateral_tokens: u64::from_le_bytes(*deposited_collateral_tokens),
            collateral_reserve: Pubkey::new_from_array(*collateral_supply),
            cumulative_borrow_rate_wads: unpack_decimal(cumulative_borrow_rate),
            borrowed_liquidity_wads: unpack_decimal(borrowed_liquidity_wads),
            borrow_reserve: Pubkey::new_from_array(*borrow_reserve),
            token_mint: Pubkey::new_from_array(*token_mint),
        })
    }

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, OBLIGATION_LEN];
        let (
            version,
            deposited_collateral_tokens,
            collateral_supply,
            cumulative_borrow_rate,
            borrowed_liquidity_wads,
            borrow_reserve,
            token_mint,
            _padding,
        ) = mut_array_refs![output, 1, 8, 32, 16, 16, 32, 32, 128];

        *version = self.version.to_le_bytes();
        *deposited_collateral_tokens = self.deposited_collateral_tokens.to_le_bytes();
        collateral_supply.copy_from_slice(self.collateral_reserve.as_ref());
        pack_decimal(self.cumulative_borrow_rate_wads, cumulative_borrow_rate);
        pack_decimal(self.borrowed_liquidity_wads, borrowed_liquidity_wads);
        borrow_reserve.copy_from_slice(self.borrow_reserve.as_ref());
        token_mint.copy_from_slice(self.token_mint.as_ref());
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::math::TryAdd;
    use proptest::prelude::*;

    const MAX_COMPOUNDED_INTEREST: u64 = 100; // 10,000%

    #[test]
    fn obligation_accrue_interest_failure() {
        assert_eq!(
            Obligation {
                cumulative_borrow_rate_wads: Decimal::zero(),
                ..Obligation::default()
            }
            .accrue_interest(Decimal::one()),
            Err(LendingError::MathOverflow.into())
        );

        assert_eq!(
            Obligation {
                cumulative_borrow_rate_wads: Decimal::from(2u64),
                ..Obligation::default()
            }
            .accrue_interest(Decimal::one()),
            Err(LendingError::NegativeInterestRate.into())
        );

        assert_eq!(
            Obligation {
                cumulative_borrow_rate_wads: Decimal::one(),
                borrowed_liquidity_wads: Decimal::from(u64::MAX),
                ..Obligation::default()
            }
            .accrue_interest(Decimal::from(10 * MAX_COMPOUNDED_INTEREST)),
            Err(LendingError::MathOverflow.into())
        );
    }

    // Creates rates (r1, r2) where 0 < r1 <= r2 <= 100*r1
    prop_compose! {
        fn cumulative_rates()(rate in 1..=u128::MAX)(
            current_rate in Just(rate),
            max_new_rate in rate..=rate.saturating_mul(MAX_COMPOUNDED_INTEREST as u128)
        ) -> (u128, u128) {
            (current_rate, max_new_rate)
        }
    }

    const MAX_BORROWED: u128 = u64::MAX as u128 * WAD as u128;

    // Creates liquidity amounts (repay, borrow) where repay < borrow
    prop_compose! {
        fn repay_partial_amounts()(repay in 1..=u64::MAX)(
            liquidity_amount in Just(repay),
            borrowed_liquidity in (WAD as u128 * repay as u128 + 1)..=MAX_BORROWED
        ) -> (u64, u128) {
            (liquidity_amount, borrowed_liquidity)
        }
    }

    // Creates liquidity amounts (repay, borrow) where repay >= borrow
    prop_compose! {
        fn repay_full_amounts()(repay in 1..=u64::MAX)(
            liquidity_amount in Just(repay),
            borrowed_liquidity in 0..=(WAD as u128 * repay as u128)
        ) -> (u64, u128) {
            (liquidity_amount, borrowed_liquidity)
        }
    }

    // Creates collateral amounts (collateral, obligation tokens) where c <= ot
    prop_compose! {
        fn collateral_amounts()(collateral in 1..=u64::MAX)(
            deposited_collateral_tokens in Just(collateral),
            obligation_tokens in collateral..=u64::MAX
        ) -> (u64, u64) {
            (deposited_collateral_tokens, obligation_tokens)
        }
    }

    proptest! {
        #[test]
        fn repay_partial(
            (liquidity_amount, borrowed_liquidity) in repay_partial_amounts(),
            (deposited_collateral_tokens, obligation_tokens) in collateral_amounts(),
        ) {
            let borrowed_liquidity_wads = Decimal::from_scaled_val(borrowed_liquidity);
            let mut state = Obligation { deposited_collateral_tokens, borrowed_liquidity_wads, ..Obligation::default() };

            let repay_result = state.repay(liquidity_amount, obligation_tokens)?;
            assert!(repay_result.decimal_repay_amount <= Decimal::from(repay_result.integer_repay_amount));
            assert!(repay_result.collateral_withdraw_amount < deposited_collateral_tokens);
            assert!(repay_result.obligation_token_amount < obligation_tokens);
            assert!(state.borrowed_liquidity_wads < borrowed_liquidity_wads);
            assert!(state.borrowed_liquidity_wads > Decimal::zero());
            assert!(state.deposited_collateral_tokens > 0);

            let obligation_token_rate = Decimal::from(repay_result.obligation_token_amount).try_div(Decimal::from(obligation_tokens))?;
            let collateral_withdraw_rate = Decimal::from(repay_result.collateral_withdraw_amount).try_div(Decimal::from(deposited_collateral_tokens))?;
            assert!(obligation_token_rate <= collateral_withdraw_rate);
        }

        #[test]
        fn repay_full(
            (liquidity_amount, borrowed_liquidity) in repay_full_amounts(),
            (deposited_collateral_tokens, obligation_tokens) in collateral_amounts(),
        ) {
            let borrowed_liquidity_wads = Decimal::from_scaled_val(borrowed_liquidity);
            let mut state = Obligation { deposited_collateral_tokens, borrowed_liquidity_wads, ..Obligation::default() } ;

            let repay_result = state.repay(liquidity_amount, obligation_tokens)?;
            assert!(repay_result.decimal_repay_amount <= Decimal::from(repay_result.integer_repay_amount));
            assert_eq!(repay_result.collateral_withdraw_amount, deposited_collateral_tokens);
            assert_eq!(repay_result.obligation_token_amount, obligation_tokens);
            assert_eq!(repay_result.decimal_repay_amount, borrowed_liquidity_wads);
            assert_eq!(state.borrowed_liquidity_wads, Decimal::zero());
            assert_eq!(state.deposited_collateral_tokens, 0);
        }

        #[test]
        fn accrue_interest(
            borrowed_liquidity in 0..=u64::MAX,
            (current_borrow_rate, new_borrow_rate) in cumulative_rates(),
        ) {
            let borrowed_liquidity_wads = Decimal::from(borrowed_liquidity);
            let cumulative_borrow_rate_wads = Decimal::one().try_add(Decimal::from_scaled_val(current_borrow_rate))?;
            let mut state = Obligation { cumulative_borrow_rate_wads, borrowed_liquidity_wads, ..Obligation::default() };

            let next_cumulative_borrow_rate = Decimal::one().try_add(Decimal::from_scaled_val(new_borrow_rate))?;
            state.accrue_interest(next_cumulative_borrow_rate)?;

            if next_cumulative_borrow_rate > cumulative_borrow_rate_wads {
                assert!(state.borrowed_liquidity_wads > borrowed_liquidity_wads);
            } else {
                assert!(state.borrowed_liquidity_wads == borrowed_liquidity_wads);
            }
        }
    }
}
