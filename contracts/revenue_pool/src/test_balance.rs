use crate::{RevenuePool, RevenuePoolClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_balance_uninitialized_panics() {
    let env = Env::default();
    let addr = env.register(RevenuePool, ());
    let client = RevenuePoolClient::new(&env, &addr);

    let result = client.try_balance();
    assert!(result.is_err());
}

#[test]
fn test_receive_payment_unauthorized_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let usdc = Address::generate(&env);
    let attacker = Address::generate(&env);

    let addr = env.register(RevenuePool, ());
    let client = RevenuePoolClient::new(&env, &addr);

    client.init(&admin, &usdc);

    let result = client.try_receive_payment(&attacker, &100, &false);
    assert!(result.is_err());
}
