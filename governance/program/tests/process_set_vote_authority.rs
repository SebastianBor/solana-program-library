#![cfg(feature = "test-bpf")]

use solana_program::{instruction::AccountMeta, pubkey::Pubkey};
use solana_program_test::*;

mod program_test;

use program_test::*;
use solana_sdk::signature::Signer;
use spl_governance::{error::GovernanceError, instruction::set_vote_authority};

#[tokio::test]
async fn test_set_governance_vote_authority() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_new().await;
    let realm_cookie = governance_test.with_realm().await;
    let voter_record_cookie = governance_test
        .with_initial_governance_token_deposit(&realm_cookie)
        .await;

    // Act
    governance_test
        .with_governance_vote_authority(&realm_cookie, &voter_record_cookie)
        .await;

    // Assert
    let voter_record = governance_test
        .get_voter_record_account(&voter_record_cookie.address)
        .await;

    assert_eq!(
        voter_record_cookie.vote_authority.pubkey(),
        voter_record.vote_authority
    );
}

#[tokio::test]
async fn test_set_council_vote_authority() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_new().await;
    let realm_cookie = governance_test.with_realm().await;
    let voter_record_cookie = governance_test
        .with_initial_council_token_deposit(&realm_cookie)
        .await;

    // Act
    governance_test
        .with_council_vote_authority(&realm_cookie, &voter_record_cookie)
        .await;

    // Assert
    let voter_record = governance_test
        .get_voter_record_account(&voter_record_cookie.address)
        .await;

    assert_eq!(
        voter_record_cookie.vote_authority.pubkey(),
        voter_record.vote_authority
    );
}

#[tokio::test]
async fn test_set_governance_vote_authority_for_owner_must_sign_error() {
    // Arrange
    let mut governance_test = GovernanceProgramTest::start_new().await;
    let realm_cookie = governance_test.with_realm().await;
    let voter_record_cookie = governance_test
        .with_initial_governance_token_deposit(&realm_cookie)
        .await;

    let hacker_vote_authority = Pubkey::new_unique();

    let mut instruction = set_vote_authority(
        &realm_cookie.address,
        &realm_cookie.governance_mint,
        &hacker_vote_authority,
        &voter_record_cookie.token_owner.pubkey(),
    )
    .unwrap();

    instruction.accounts[0] =
        AccountMeta::new_readonly(voter_record_cookie.token_owner.pubkey(), false);

    // Act
    let err = governance_test
        .process_transaction(&[instruction], None)
        .await
        .err()
        .unwrap();

    // Assert
    assert_eq!(err, GovernanceError::GoverningTokenOwnerMustSign.into());
}
