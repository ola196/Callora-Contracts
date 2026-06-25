extern crate std;

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env, Vec};
use crate::{CalloraSettlement, CalloraSettlementClient, MAX_BATCH_SIZE};

fn setup_contract() -> (Env, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let vault = Address::generate(&env);
    let third_party = Address::generate(&env);

    let contract_id = env.register(CalloraSettlement, ());
    let client = CalloraSettlementClient::new(&env, &contract_id);
    client.init(&admin, &vault);

    (env, contract_id, admin, vault, third_party)
}

#[test]
fn test_batch_receive_fuzz() {
    let (env, addr, _, vault, _) = setup_contract();
    let client = CalloraSettlementClient::new(&env, &addr);

    // Deterministic seeded PRNG
    let mut state: u64 = 0xdeadbeef;
    let mut next_u32 = || -> u32 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state as u32
    };

    let dev1 = Address::generate(&env);
    let dev2 = Address::generate(&env);
    let devs = [&dev1, &dev2];

    for _trace in 0..256 {
        let batch_size = (next_u32() % MAX_BATCH_SIZE) + 1;
        
        let mut items: Vec<(Option<Address>, bool, i128)> = Vec::new(&env);
        let mut expected_pool_increase = 0i128;
        let mut expected_dev_increase = std::vec::Vec::new();
        expected_dev_increase.push(0i128); // dev1
        expected_dev_increase.push(0i128); // dev2

        let mut will_fail = false;

        for _ in 0..batch_size {
            let is_pool = (next_u32() % 2) == 0;
            let amount = (next_u32() % 1000) as i128 - 100; // allow negative amounts

            if amount <= 0 {
                will_fail = true;
            }

            if is_pool {
                // For pool, developer must be None
                let dev_valid = (next_u32() % 10) != 0; // 90% valid
                let dev = if dev_valid {
                    None
                } else {
                    will_fail = true;
                    Some(devs[(next_u32() % 2) as usize].clone())
                };
                items.push_back((dev, true, amount));
                if !will_fail {
                    expected_pool_increase += amount;
                }
            } else {
                // For developer, developer must be Some
                let dev_valid = (next_u32() % 10) != 0; // 90% valid
                let dev_idx = (next_u32() % 2) as usize;
                let dev = if dev_valid {
                    Some(devs[dev_idx].clone())
                } else {
                    will_fail = true;
                    None
                };
                items.push_back((dev, false, amount));
                if !will_fail {
                    expected_dev_increase[dev_idx] += amount;
                }
            }
        }

        let old_pool = client.get_global_pool().total_balance;
        let old_dev1 = client.get_developer_balance(&dev1);
        let old_dev2 = client.get_developer_balance(&dev2);

        let res = client.try_batch_receive_payment(&vault, &items);

        if will_fail {
            assert!(res.is_err(), "Expected failure but succeeded! trace: {}", _trace);
            // Verify no state mutated
            assert_eq!(client.get_global_pool().total_balance, old_pool);
            assert_eq!(client.get_developer_balance(&dev1), old_dev1);
            assert_eq!(client.get_developer_balance(&dev2), old_dev2);
        } else {
            assert!(res.is_ok(), "Expected success but failed! trace: {}", _trace);
            // Verify state mutated according to conservation invariant
            assert_eq!(client.get_global_pool().total_balance, old_pool + expected_pool_increase);
            assert_eq!(client.get_developer_balance(&dev1), old_dev1 + expected_dev_increase[0]);
            assert_eq!(client.get_developer_balance(&dev2), old_dev2 + expected_dev_increase[1]);
        }
    }
}
