extern crate std;

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{token, Address, Env, Vec};

use callora_revenue_pool::{RevenuePool, RevenuePoolClient, MAX_BATCH_SIZE};

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

fn create_pool(env: &Env) -> (Address, RevenuePoolClient<'_>) {
    let address = env.register(RevenuePool, ());
    let client = RevenuePoolClient::new(env, &address);
    (address, client)
}

fn gas(label: &str, env: &Env, cpu_before: u64, mem_before: u64) {
    let budget = env.cost_estimate().budget();
    let cpu = budget.cpu_instruction_cost().saturating_sub(cpu_before);
    let mem = budget.memory_bytes_cost().saturating_sub(mem_before);
    std::println!("GAS| {} | {} | {} | {}", label, cpu, mem, label);
}

#[test]
fn bench_revenue_pool_init() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let (_, client) = create_pool(&env);
    let (usdc, _, _) = create_usdc(&env, &admin);

    let cpu_before = env.cost_estimate().budget().cpu_instruction_cost();
    let mem_before = env.cost_estimate().budget().memory_bytes_cost();
    client.init(&admin, &usdc);
    gas("revenue_pool::init", &env, cpu_before, mem_before);
}

#[test]
fn bench_revenue_pool_distribute() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (pool_addr, client) = create_pool(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &admin);
    client.init(&admin, &usdc_address);
    usdc_admin.mint(&pool_addr, &10_000);

    let cpu_before = env.cost_estimate().budget().cpu_instruction_cost();
    let mem_before = env.cost_estimate().budget().memory_bytes_cost();
    client.distribute(&admin, &developer, &1000);
    gas("revenue_pool::distribute", &env, cpu_before, mem_before);
}

fn bench_batch_distribute(k: u32) -> (u64, u64) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let (pool_addr, client) = create_pool(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &admin);
    client.init(&admin, &usdc_address);
    let total = (k as i128) * 1000;
    usdc_admin.mint(&pool_addr, &total);

    let mut payments: Vec<(Address, i128)> = Vec::new(&env);
    for _ in 0..k {
        payments.push_back((Address::generate(&env), 1000i128));
    }

    let cpu_before = env.cost_estimate().budget().cpu_instruction_cost();
    let mem_before = env.cost_estimate().budget().memory_bytes_cost();
    client.batch_distribute(&admin, &payments);
    let cpu = env.cost_estimate().budget().cpu_instruction_cost().saturating_sub(cpu_before);
    let mem = env.cost_estimate().budget().memory_bytes_cost().saturating_sub(mem_before);
    (cpu, mem)
}

#[test]
fn bench_revenue_pool_batch_distribute_k1() {
    let (cpu, mem) = bench_batch_distribute(1);
    std::println!("GAS| revenue_pool::batch_distribute_k1 | {} | {} | revenue_pool::batch_distribute[k=1]", cpu, mem);
}

#[test]
fn bench_revenue_pool_batch_distribute_k10() {
    let (cpu, mem) = bench_batch_distribute(10);
    std::println!("GAS| revenue_pool::batch_distribute_k10 | {} | {} | revenue_pool::batch_distribute[k=10]", cpu, mem);
}

#[test]
fn bench_revenue_pool_batch_distribute_k50() {
    let (cpu, mem) = bench_batch_distribute(MAX_BATCH_SIZE);
    std::println!("GAS| revenue_pool::batch_distribute_k50 | {} | {} | revenue_pool::batch_distribute[k=50]", cpu, mem);
}

#[test]
fn bench_revenue_pool_receive_payment() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let (_, client) = create_pool(&env);
    let (usdc, _, _) = create_usdc(&env, &admin);
    client.init(&admin, &usdc);

    let cpu_before = env.cost_estimate().budget().cpu_instruction_cost();
    let mem_before = env.cost_estimate().budget().memory_bytes_cost();
    client.receive_payment(&admin, &1000, &true);
    gas("revenue_pool::receive_payment", &env, cpu_before, mem_before);
}
