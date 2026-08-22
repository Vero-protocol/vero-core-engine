//! Treasury snapshot integration tests.

#[cfg(test)]
mod tests {

    use crate::treasury;
    use crate::types::TriggerKind;
    use soroban_sdk::{contract, contractimpl, testutils::Address as _, Address, Env, Map, Symbol, Val};


    #[contract]
    pub struct TestContract;

    #[contractimpl]
    impl TestContract {}

    fn setup() -> (Env, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TestContract);
        let admin = Address::generate(&env);
        env.as_contract(&contract_id, || treasury::init(&env, admin.clone()));
        (env, contract_id, admin)
    }

    #[test]
    fn records_and_retrieves_snapshot() {
        let (env, contract_id, admin) = setup();
        env.as_contract(&contract_id, || {
            let ctx: Map<Symbol, Val> = Map::new(&env);
            let id = treasury::record_snapshot(&env, &admin, 1000, 5, TriggerKind::Deposit, ctx);
            let snap = treasury::get_snapshot(&env, id).unwrap();
            assert_eq!(snap.id, 1);
            assert_eq!(snap.total_balance, 1000);
            assert!(treasury::verify_snapshot(&env, &snap));
        });
    }

    #[test]
    fn recent_snapshots_are_newest_first() {
        let (env, contract_id, admin) = setup();
        for i in 0..3 {
            env.as_contract(&contract_id, || {
                let ctx: Map<Symbol, Val> = Map::new(&env);
                treasury::record_snapshot(&env, &admin, 100 + i, 1, TriggerKind::Manual, ctx);
            });
        }
        env.as_contract(&contract_id, || {
            let ids = treasury::get_recent_snapshots(&env, 2);
            assert_eq!(ids.get(0).unwrap(), 3);
            assert_eq!(ids.get(1).unwrap(), 2);
        });
    }

    #[test]
    #[should_panic]
    fn negative_balance_is_rejected() {
        let (env, contract_id, admin) = setup();
        env.as_contract(&contract_id, || {
            let ctx: Map<Symbol, Val> = Map::new(&env);
            treasury::record_snapshot(&env, &admin, -1, 0, TriggerKind::Other, ctx);
        });
    }

    #[test]
    #[should_panic]
    fn record_snapshot_rejects_unauthorized_caller() {
        let (env, contract_id, _admin) = setup();
        let rogue = Address::generate(&env);
        env.as_contract(&contract_id, || {
            let ctx: Map<Symbol, Val> = Map::new(&env);
            treasury::record_snapshot(&env, &rogue, 1000, 1, TriggerKind::Manual, ctx);
        });
    }
}
