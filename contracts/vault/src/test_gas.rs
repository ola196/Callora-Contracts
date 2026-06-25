extern crate std;

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{token, Address, Env, Symbol, Vec};

use super::*;
use callora_settlement::CalloraSettlement;

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

fn gas(label: &str, env: &Env, cpu_before: u64, mem_before: u64) {
    let budget = env.cost_estimate().budget();
    let cpu = budget.cpu_instruction_cost().saturating_sub(cpu_before);
    let mem = budget.memory_bytes_cost().saturating_sub(mem_before);
    std::println!("GAS| {} | {} | {} | {}", label, cpu, mem, label);
}

fn cpu_mem(env: &Env, cpu_before: u64, mem_before: u64) -> (u64, u64) {
    let budget = env.cost_estimate().budget();
    let cpu = budget.cpu_instruction_cost().saturating_sub(cpu_before);
    let mem = budget.memory_bytes_cost().saturating_sub(mem_before);
    (cpu, mem)
}

fn init_vault(
    env: &Env,
    client: &CalloraVaultClient,
    owner: &Address,
    usdc: &Address,
    vault_address: &Address,
    usdc_admin: &token::StellarAssetClient,
) {
    env.mock_all_auths();
    fund_vault(usdc_admin, vault_address, 100_000);
    client.init(owner, usdc, &Some(100_000), &None, &None, &None, &None);
}

#[test]
fn bench_vault_init() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let (vault_address, client) = create_vault(&env);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);
    env.mock_all_auths();
    fund_vault(&usdc_admin, &vault_address, 1000);

    let budget = env.cost_estimate().budget();
    let cpu_before = budget.cpu_instruction_cost();
    let mem_before = budget.memory_bytes_cost();
    client.init(&owner, &usdc, &Some(1000), &None, &None, &None, &None);
    gas("vault::init", &env, cpu_before, mem_before);
}

#[test]
fn bench_vault_deposit() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let (vault_address, client) = create_vault(&env);
    let (usdc, usdc_client, usdc_admin) = create_usdc(&env, &owner);
    init_vault(&env, &client, &owner, &usdc, &vault_address, &usdc_admin);
    usdc_admin.mint(&owner, &1000);
    usdc_client.approve(&owner, &vault_address, &1000, &1000);

    let cpu_before = env.cost_estimate().budget().cpu_instruction_cost();
    let mem_before = env.cost_estimate().budget().memory_bytes_cost();
    client.deposit(&owner, &500);
    gas("vault::deposit", &env, cpu_before, mem_before);
}

#[test]
fn bench_vault_deduct() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let (vault_address, client) = create_vault(&env);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);
    init_vault(&env, &client, &owner, &usdc, &vault_address, &usdc_admin);
    let settlement = create_settlement(&env, &owner, &vault_address);
    client.set_settlement(&owner, &settlement);

    let cpu_before = env.cost_estimate().budget().cpu_instruction_cost();
    let mem_before = env.cost_estimate().budget().memory_bytes_cost();
    client.deduct(&owner, &1000, &None);
    gas("vault::deduct", &env, cpu_before, mem_before);
}

fn bench_batch_deduct(k: u32) -> (u64, u64) {
    let env = Env::default();
    let owner = Address::generate(&env);
    let (vault_address, client) = create_vault(&env);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);
    init_vault(&env, &client, &owner, &usdc, &vault_address, &usdc_admin);
    let settlement = create_settlement(&env, &owner, &vault_address);
    client.set_settlement(&owner, &settlement);
    let mut items: Vec<DeductItem> = Vec::new(&env);
    for _ in 0..k {
        items.push_back(DeductItem {
            amount: 100,
            request_id: None,
        });
    }

    let cpu_before = env.cost_estimate().budget().cpu_instruction_cost();
    let mem_before = env.cost_estimate().budget().memory_bytes_cost();
    client.batch_deduct(&owner, &items);
    let cpu = env.cost_estimate().budget().cpu_instruction_cost().saturating_sub(cpu_before);
    let mem = env.cost_estimate().budget().memory_bytes_cost().saturating_sub(mem_before);
    (cpu, mem)
}

#[test]
fn bench_vault_batch_deduct_k1() {
    let (cpu, mem) = bench_batch_deduct(1);
    std::println!("GAS| vault::batch_deduct_k1 | {} | {} | vault::batch_deduct[k=1]", cpu, mem);
}

#[test]
fn bench_vault_batch_deduct_k10() {
    let (cpu, mem) = bench_batch_deduct(10);
    std::println!("GAS| vault::batch_deduct_k10 | {} | {} | vault::batch_deduct[k=10]", cpu, mem);
}

#[test]
fn bench_vault_batch_deduct_k50() {
    let (cpu, mem) = bench_batch_deduct(MAX_BATCH_SIZE);
    std::println!("GAS| vault::batch_deduct_k50 | {} | {} | vault::batch_deduct[k=50]", cpu, mem);
}

#[test]
fn bench_vault_withdraw() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let (vault_address, client) = create_vault(&env);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);
    init_vault(&env, &client, &owner, &usdc, &vault_address, &usdc_admin);

    let cpu_before = env.cost_estimate().budget().cpu_instruction_cost();
    let mem_before = env.cost_estimate().budget().memory_bytes_cost();
    client.withdraw(&1000);
    gas("vault::withdraw", &env, cpu_before, mem_before);
}

#[test]
fn bench_vault_withdraw_to() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let recipient = Address::generate(&env);
    let (vault_address, client) = create_vault(&env);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);
    init_vault(&env, &client, &owner, &usdc, &vault_address, &usdc_admin);

    let cpu_before = env.cost_estimate().budget().cpu_instruction_cost();
    let mem_before = env.cost_estimate().budget().memory_bytes_cost();
    client.withdraw_to(&recipient, &1000);
    gas("vault::withdraw_to", &env, cpu_before, mem_before);
}

#[test]
fn bench_vault_distribute() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let recipient = Address::generate(&env);
    let (vault_address, client) = create_vault(&env);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);
    init_vault(&env, &client, &owner, &usdc, &vault_address, &usdc_admin);

    let cpu_before = env.cost_estimate().budget().cpu_instruction_cost();
    let mem_before = env.cost_estimate().budget().memory_bytes_cost();
    client.distribute(&owner, &recipient, &1000);
    gas("vault::distribute", &env, cpu_before, mem_before);
}
