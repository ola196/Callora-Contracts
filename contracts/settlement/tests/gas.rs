extern crate std;

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{token, Address, Env, Vec};

use callora_settlement::{CalloraSettlement, CalloraSettlementClient};

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

fn gas(label: &str, env: &Env, cpu_before: u64, mem_before: u64) {
    let budget = env.cost_estimate().budget();
    let cpu = budget.cpu_instruction_cost().saturating_sub(cpu_before);
    let mem = budget.memory_bytes_cost().saturating_sub(mem_before);
    std::println!("GAS| {} | {} | {} | {}", label, cpu, mem, label);
}

#[test]
fn bench_settlement_init() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let vault = Address::generate(&env);
    let addr = env.register(CalloraSettlement, ());
    let client = CalloraSettlementClient::new(&env, &addr);

    let cpu_before = env.cost_estimate().budget().cpu_instruction_cost();
    let mem_before = env.cost_estimate().budget().memory_bytes_cost();
    client.init(&admin, &vault);
    gas("settlement::init", &env, cpu_before, mem_before);
}

#[test]
fn bench_settlement_receive_payment() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let vault = Address::generate(&env);
    let addr = env.register(CalloraSettlement, ());
    let client = CalloraSettlementClient::new(&env, &addr);
    client.init(&admin, &vault);

    let cpu_before = env.cost_estimate().budget().cpu_instruction_cost();
    let mem_before = env.cost_estimate().budget().memory_bytes_cost();
    client.receive_payment(&vault, &1000i128, &true, &None);
    gas("settlement::receive_payment", &env, cpu_before, mem_before);
}

fn bench_batch_receive_payment(k: u32) -> (u64, u64) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let vault = Address::generate(&env);
    let addr = env.register(CalloraSettlement, ());
    let client = CalloraSettlementClient::new(&env, &addr);
    client.init(&admin, &vault);

    let mut items: Vec<(Address, i128)> = Vec::new(&env);
    for _ in 0..k {
        items.push_back((Address::generate(&env), 100i128));
    }

    let cpu_before = env.cost_estimate().budget().cpu_instruction_cost();
    let mem_before = env.cost_estimate().budget().memory_bytes_cost();
    client.batch_receive_payment(&vault, &items);
    let cpu = env
        .cost_estimate()
        .budget()
        .cpu_instruction_cost()
        .saturating_sub(cpu_before);
    let mem = env
        .cost_estimate()
        .budget()
        .memory_bytes_cost()
        .saturating_sub(mem_before);
    (cpu, mem)
}

#[test]
fn bench_settlement_batch_receive_payment_k1() {
    let (cpu, mem) = bench_batch_receive_payment(1);
    std::println!("GAS| settlement::batch_receive_payment_k1 | {} | {} | settlement::batch_receive_payment[k=1]", cpu, mem);
}

#[test]
fn bench_settlement_batch_receive_payment_k10() {
    let (cpu, mem) = bench_batch_receive_payment(10);
    std::println!("GAS| settlement::batch_receive_payment_k10 | {} | {} | settlement::batch_receive_payment[k=10]", cpu, mem);
}

#[test]
fn bench_settlement_batch_receive_payment_k50() {
    let (cpu, mem) = bench_batch_receive_payment(callora_settlement::MAX_BATCH_SIZE);
    std::println!("GAS| settlement::batch_receive_payment_k50 | {} | {} | settlement::batch_receive_payment[k=50]", cpu, mem);
}

#[test]
fn bench_settlement_withdraw_developer_balance() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let vault = Address::generate(&env);
    let developer = Address::generate(&env);
    let addr = env.register(CalloraSettlement, ());
    let client = CalloraSettlementClient::new(&env, &addr);
    let (usdc_address, _, usdc_admin_client) = create_usdc(&env, &admin);
    client.init(&admin, &vault);
    client.set_usdc_token(&admin, &usdc_address);
    client.receive_payment(&vault, &1000i128, &false, &Some(developer.clone()));
    usdc_admin_client.mint(&addr, &1000i128);

    let cpu_before = env.cost_estimate().budget().cpu_instruction_cost();
    let mem_before = env.cost_estimate().budget().memory_bytes_cost();
    client.withdraw_developer_balance(&developer, &500i128);
    gas(
        "settlement::withdraw_developer_balance",
        &env,
        cpu_before,
        mem_before,
    );
}
