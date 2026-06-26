extern crate std;

use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::{token, Address, Env, IntoVal, Symbol, Vec};

use super::*;

use callora_settlement::CalloraSettlement;

// ---------------------------------------------------------------------------
// Test helpers (self-contained, matching test.rs conventions)
// ---------------------------------------------------------------------------

fn create_usdc<'a>(
    env: &'a Env,
    admin: &Address,
) -> (Address, token::Client<'a>, token::StellarAssetClient<'a>) {
    let contract_address = env.register_stellar_asset_contract_v2(admin.clone());
    let address = contract_address.address();
    let client = token::Client::new(env, &address);
    let admin_client = token::StellarAssetClient::new(env, &address);
    (address, client, admin_client)
}

fn create_vault(env: &Env) -> (Address, CalloraVaultClient<'_>) {
    let address = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(env, &address);
    (address, client)
}

fn create_settlement(env: &Env, admin: &Address, vault_address: &Address) -> Address {
    let settlement_address = env.register(CalloraSettlement, ());
    let settlement_client =
        callora_settlement::CalloraSettlementClient::new(env, &settlement_address);
    env.mock_all_auths();
    settlement_client.init(admin, vault_address);
    settlement_address
}

fn fund_vault(
    usdc_admin_client: &token::StellarAssetClient,
    vault_address: &Address,
    amount: i128,
) {
    usdc_admin_client.mint(vault_address, &amount);
}

/// Fully initialize a vault with USDC, settlement, and an initial balance.
/// Returns (vault_address, client, owner, usdc_admin_client, settlement_address).
fn setup_vault(env: &Env) -> (Address, CalloraVaultClient, Address, token::StellarAssetClient, Address) {
    let owner = Address::generate(env);
    let (vault_address, client) = create_vault(env);
    let (usdc, _usdc_client, usdc_admin) = create_usdc(env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &vault_address, 1_000);
    client.init(
        &owner,
        &usdc,
        &Some(1_000),
        &None,
        &None,
        &None,
        &None,
    );

    let settlement = create_settlement(env, &owner, &vault_address);
    client.set_settlement(&owner, &settlement);

    (vault_address, client, owner, usdc_admin, settlement)
}

// ---------------------------------------------------------------------------
// BLOCKED when paused — deposit, deduct, batch_deduct
// ---------------------------------------------------------------------------

/// deposit must return VaultError::Paused when the vault is paused.
#[test]
fn deposit_blocked_when_paused() {
    let env = Env::default();
    let (_, client, owner, usdc_admin, _) = setup_vault(&env);

    // Pre-mint and approve so the only failure reason is the pause gate.
    usdc_admin.mint(&owner, &100);
    let (usdc, usdc_client, _) = create_usdc(&env, &owner);
    usdc_client.approve(&owner, &client.address, &100, &1000);

    client.pause(&owner);
    assert!(client.is_paused());

    let result = client.try_deposit(&owner, &100);
    assert_eq!(
        result,
        Err(Ok(VaultError::Paused)),
        "deposit must be blocked when vault is paused"
    );
}

/// deduct must return VaultError::Paused when the vault is paused.
#[test]
fn deduct_blocked_when_paused() {
    let env = Env::default();
    let (_, client, owner, _, _) = setup_vault(&env);

    client.pause(&owner);
    assert!(client.is_paused());

    let result = client.try_deduct(&owner, &100, &None);
    assert_eq!(
        result,
        Err(Ok(VaultError::Paused)),
        "deduct must be blocked when vault is paused"
    );
}

/// batch_deduct must return VaultError::Paused when the vault is paused.
#[test]
fn batch_deduct_blocked_when_paused() {
    let env = Env::default();
    let (_, client, owner, _, _) = setup_vault(&env);

    client.pause(&owner);
    assert!(client.is_paused());

    let items = Vec::from_array(
        &env,
        [
            DeductItem {
                amount: 50,
                request_id: None,
            },
            DeductItem {
                amount: 50,
                request_id: None,
            },
        ],
    );

    let result = client.try_batch_deduct(&owner, &items);
    assert_eq!(
        result,
        Err(Ok(VaultError::Paused)),
        "batch_deduct must be blocked when vault is paused"
    );
}

// ---------------------------------------------------------------------------
// ALLOWED when paused — withdraw, withdraw_to, distribute
// ---------------------------------------------------------------------------

/// withdraw must succeed when the vault is paused (emergency recovery).
#[test]
fn withdraw_allowed_when_paused() {
    let env = Env::default();
    let (vault_address, client, owner, usdc_admin, _) = setup_vault(&env);
    let (_, usdc_client, _) = create_usdc(&env, &owner);

    client.pause(&owner);
    assert!(client.is_paused());

    let remaining = client.withdraw(&200);
    assert_eq!(remaining, 800);
    assert_eq!(client.balance(), 800);
    assert_eq!(usdc_client.balance(&owner), 200);
    assert_eq!(usdc_client.balance(&vault_address), 800);
}

/// withdraw_to must succeed when the vault is paused (emergency recovery).
#[test]
fn withdraw_to_allowed_when_paused() {
    let env = Env::default();
    let (vault_address, client, owner, usdc_admin, _) = setup_vault(&env);
    let (_, usdc_client, _) = create_usdc(&env, &owner);
    let recipient = Address::generate(&env);

    client.pause(&owner);
    assert!(client.is_paused());

    let remaining = client.withdraw_to(&recipient, &200);
    assert_eq!(remaining, 800);
    assert_eq!(client.balance(), 800);
    assert_eq!(usdc_client.balance(&recipient), 200);
    assert_eq!(usdc_client.balance(&vault_address), 800);
}

/// distribute must succeed when the vault is paused (emergency recovery of untracked surplus).
#[test]
fn distribute_allowed_when_paused() {
    let env = Env::default();
    let (vault_address, client, owner, usdc_admin, _) = setup_vault(&env);
    let (_, usdc_client, _) = create_usdc(&env, &owner);
    let recipient = Address::generate(&env);

    client.pause(&owner);
    assert!(client.is_paused());

    // distribute checks on-ledger USDC balance, not meta.balance
    client.distribute(&owner, &recipient, &500);

    assert_eq!(usdc_client.balance(&recipient), 500);
    assert_eq!(usdc_client.balance(&vault_address), 500);
    // meta.balance unchanged (distribute does not touch tracked balance)
    assert_eq!(client.balance(), 1_000);
}

// ---------------------------------------------------------------------------
// ALLOWED when unpaused — happy path verification for the full matrix
// ---------------------------------------------------------------------------

/// deposit must succeed when the vault is unpaused.
#[test]
fn deposit_allowed_when_unpaused() {
    let env = Env::default();
    let (_, client, owner, usdc_admin, _) = setup_vault(&env);
    let (_, usdc_client, _) = create_usdc(&env, &owner);

    assert!(!client.is_paused());

    usdc_admin.mint(&owner, &200);
    usdc_client.approve(&owner, &client.address, &200, &1000);

    let new_balance = client.deposit(&owner, &200);
    assert_eq!(new_balance, 1_200);
    assert_eq!(client.balance(), 1_200);
}

/// deduct must succeed when the vault is unpaused.
#[test]
fn deduct_allowed_when_unpaused() {
    let env = Env::default();
    let (_, client, owner, _, _) = setup_vault(&env);

    assert!(!client.is_paused());

    let remaining = client.deduct(&owner, &200, &None);
    assert_eq!(remaining, 800);
    assert_eq!(client.balance(), 800);
}

/// batch_deduct must succeed when the vault is unpaused.
#[test]
fn batch_deduct_allowed_when_unpaused() {
    let env = Env::default();
    let (_, client, owner, _, _) = setup_vault(&env);

    assert!(!client.is_paused());

    let items = Vec::from_array(
        &env,
        [
            DeductItem {
                amount: 100,
                request_id: None,
            },
            DeductItem {
                amount: 100,
                request_id: None,
            },
        ],
    );

    let remaining = client.batch_deduct(&owner, &items);
    assert_eq!(remaining, 800);
    assert_eq!(client.balance(), 800);
}

/// withdraw must succeed when the vault is unpaused (baseline).
#[test]
fn withdraw_allowed_when_unpaused() {
    let env = Env::default();
    let (_, client, owner, _, _) = setup_vault(&env);

    assert!(!client.is_paused());

    let remaining = client.withdraw(&200);
    assert_eq!(remaining, 800);
    assert_eq!(client.balance(), 800);
}

/// withdraw_to must succeed when the vault is unpaused (baseline).
#[test]
fn withdraw_to_allowed_when_unpaused() {
    let env = Env::default();
    let (_, client, owner, _, _) = setup_vault(&env);
    let recipient = Address::generate(&env);

    assert!(!client.is_paused());

    let remaining = client.withdraw_to(&recipient, &200);
    assert_eq!(remaining, 800);
    assert_eq!(client.balance(), 800);
}

/// distribute must succeed when the vault is unpaused (baseline).
#[test]
fn distribute_allowed_when_unpaused() {
    let env = Env::default();
    let (vault_address, client, owner, usdc_admin, _) = setup_vault(&env);
    let (_, usdc_client, _) = create_usdc(&env, &owner);
    let recipient = Address::generate(&env);

    assert!(!client.is_paused());

    client.distribute(&owner, &recipient, &500);

    assert_eq!(usdc_client.balance(&recipient), 500);
    assert_eq!(usdc_client.balance(&vault_address), 500);
    assert_eq!(client.balance(), 1_000);
}

// ---------------------------------------------------------------------------
// Edge cases — pause/unpause lifecycle
// ---------------------------------------------------------------------------

/// Pausing, then unpausing, must restore deposit functionality.
#[test]
fn pause_then_unpause_restores_deposit() {
    let env = Env::default();
    let (_, client, owner, usdc_admin, _) = setup_vault(&env);
    let (_, usdc_client, _) = create_usdc(&env, &owner);

    // Pause
    client.pause(&owner);
    assert!(client.is_paused());

    // Deposit blocked
    usdc_admin.mint(&owner, &100);
    usdc_client.approve(&owner, &client.address, &100, &1000);
    assert!(client.try_deposit(&owner, &100).is_err());

    // Unpause
    client.unpause(&owner);
    assert!(!client.is_paused());

    // Deposit restored
    let new_balance = client.deposit(&owner, &100);
    assert_eq!(new_balance, 1_100);
    assert_eq!(client.balance(), 1_100);
}

/// Double-pause must return VaultError::AlreadyPaused.
#[test]
fn double_pause_fails() {
    let env = Env::default();
    let (_, client, owner, _, _) = setup_vault(&env);

    client.pause(&owner);
    let result = client.try_pause(&owner);
    assert_eq!(
        result,
        Err(Ok(VaultError::AlreadyPaused)),
        "double pause must fail with AlreadyPaused"
    );
}

/// Double-unpause must return VaultError::NotPaused.
#[test]
fn double_unpause_fails() {
    let env = Env::default();
    let (_, client, owner, _, _) = setup_vault(&env);

    // Ensure we start unpaused (default)
    assert!(!client.is_paused());
    let result = client.try_unpause(&owner);
    assert_eq!(
        result,
        Err(Ok(VaultError::NotPaused)),
        "double unpause must fail with NotPaused"
    );
}

/// Non-admin/non-owner cannot pause.
#[test]
fn pause_unauthorized_fails() {
    let env = Env::default();
    let (_, client, owner, _, _) = setup_vault(&env);
    let intruder = Address::generate(&env);

    let result = client.try_pause(&intruder);
    assert!(result.is_err(), "non-admin must not be able to pause");
}

/// Non-admin/non-owner cannot unpause.
#[test]
fn unpause_unauthorized_fails() {
    let env = Env::default();
    let (_, client, owner, _, _) = setup_vault(&env);
    let intruder = Address::generate(&env);

    client.pause(&owner);

    let result = client.try_unpause(&intruder);
    assert!(result.is_err(), "non-admin must not be able to unpause");
}

/// Balance must remain unchanged when a blocked operation is attempted.
#[test]
fn balance_unchanged_after_blocked_deposit() {
    let env = Env::default();
    let (_, client, owner, usdc_admin, _) = setup_vault(&env);
    let (_, usdc_client, _) = create_usdc(&env, &owner);

    client.pause(&owner);

    let before = client.balance();
    usdc_admin.mint(&owner, &100);
    usdc_client.approve(&owner, &client.address, &100, &1000);
    let _ = client.try_deposit(&owner, &100);
    assert_eq!(
        client.balance(),
        before,
        "balance must be unchanged after blocked deposit"
    );
}

/// Balance must remain unchanged when a blocked deduct is attempted.
#[test]
fn balance_unchanged_after_blocked_deduct() {
    let env = Env::default();
    let (_, client, owner, _, _) = setup_vault(&env);

    client.pause(&owner);

    let before = client.balance();
    let _ = client.try_deduct(&owner, &100, &None);
    assert_eq!(
        client.balance(),
        before,
        "balance must be unchanged after blocked deduct"
    );
}

/// Balance must remain unchanged when a blocked batch_deduct is attempted.
#[test]
fn balance_unchanged_after_blocked_batch_deduct() {
    let env = Env::default();
    let (_, client, owner, _, _) = setup_vault(&env);

    client.pause(&owner);

    let before = client.balance();
    let items = Vec::from_array(
        &env,
        [DeductItem {
            amount: 100,
            request_id: None,
        }],
    );
    let _ = client.try_batch_deduct(&owner, &items);
    assert_eq!(
        client.balance(),
        before,
        "balance must be unchanged after blocked batch_deduct"
    );
}

/// Pause event must be emitted with correct topics.
#[test]
fn pause_emits_event() {
    let env = Env::default();
    let (vault_address, client, owner, _, _) = setup_vault(&env);

    client.pause(&owner);

    let events = env.events().all();
    let ev = events
        .iter()
        .find(|e| {
            e.0 == vault_address
                && !e.1.is_empty()
                && {
                    let t: Symbol = e.1.get(0).unwrap().into_val(&env);
                    t == Symbol::new(&env, "vault_paused")
                }
        })
        .expect("expected vault_paused event");

    let caller_topic: Address = ev.1.get(1).unwrap().into_val(&env);
    assert_eq!(caller_topic, owner);
}

/// Unpause event must be emitted with correct topics.
#[test]
fn unpause_emits_event() {
    let env = Env::default();
    let (vault_address, client, owner, _, _) = setup_vault(&env);

    client.pause(&owner);
    client.unpause(&owner);

    let events = env.events().all();
    let ev = events
        .iter()
        .find(|e| {
            e.0 == vault_address
                && !e.1.is_empty()
                && {
                    let t: Symbol = e.1.get(0).unwrap().into_val(&env);
                    t == Symbol::new(&env, "vault_unpaused")
                }
        })
        .expect("expected vault_unpaused event");

    let caller_topic: Address = ev.1.get(1).unwrap().into_val(&env);
    assert_eq!(caller_topic, owner);
}

/// Comprehensive matrix assertion: all six entrypoints in a single scenario.
#[test]
fn full_pause_matrix_scenario() {
    let env = Env::default();
    let (vault_address, client, owner, usdc_admin, _) = setup_vault(&env);
    let (_, usdc_client, _) = create_usdc(&env, &owner);
    let recipient = Address::generate(&env);

    // --- Phase 1: Unpaused — all operations succeed -------------------------
    assert!(!client.is_paused());

    // deposit
    usdc_admin.mint(&owner, &100);
    usdc_client.approve(&owner, &client.address, &100, &1000);
    assert_eq!(client.deposit(&owner, &100), 1_100);

    // deduct
    assert_eq!(client.deduct(&owner, &50, &None), 1_050);

    // batch_deduct
    let items = Vec::from_array(
        &env,
        [DeductItem {
            amount: 50,
            request_id: None,
        }],
    );
    assert_eq!(client.batch_deduct(&owner, &items), 1_000);

    // withdraw
    assert_eq!(client.withdraw(&100), 900);

    // withdraw_to
    assert_eq!(client.withdraw_to(&recipient, &100), 800);

    // distribute
    client.distribute(&owner, &recipient, &100);
    // meta.balance unchanged by distribute
    assert_eq!(client.balance(), 800);

    // --- Phase 2: Paused — blocked operations fail, allowed succeed ---------
    client.pause(&owner);
    assert!(client.is_paused());

    // deposit blocked
    usdc_admin.mint(&owner, &100);
    usdc_client.approve(&owner, &client.address, &100, &1000);
    assert_eq!(
        client.try_deposit(&owner, &100),
        Err(Ok(VaultError::Paused))
    );

    // deduct blocked
    assert_eq!(
        client.try_deduct(&owner, &50, &None),
        Err(Ok(VaultError::Paused))
    );

    // batch_deduct blocked
    let blocked_items = Vec::from_array(
        &env,
        [DeductItem {
            amount: 50,
            request_id: None,
        }],
    );
    assert_eq!(
        client.try_batch_deduct(&owner, &blocked_items),
        Err(Ok(VaultError::Paused))
    );

    // withdraw allowed
    assert_eq!(client.withdraw(&100), 700);

    // withdraw_to allowed
    assert_eq!(client.withdraw_to(&recipient, &100), 600);

    // distribute allowed
    client.distribute(&owner, &recipient, &100);
    assert_eq!(client.balance(), 600); // still unchanged

    // --- Phase 3: Unpaused — operations restored ----------------------------
    client.unpause(&owner);
    assert!(!client.is_paused());

    // deposit restored
    usdc_admin.mint(&owner, &100);
    usdc_client.approve(&owner, &client.address, &100, &1000);
    assert_eq!(client.deposit(&owner, &100), 700);

    // deduct restored
    assert_eq!(client.deduct(&owner, &50, &None), 650);

    // batch_deduct restored
    let restored_items = Vec::from_array(
        &env,
        [DeductItem {
            amount: 50,
            request_id: None,
        }],
    );
    assert_eq!(client.batch_deduct(&owner, &restored_items), 600);
}