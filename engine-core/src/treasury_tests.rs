//! Treasury snapshot integration tests.

#[cfg(test)]
mod tests {

    use crate::treasury;
    use crate::types::TriggerKind;
    use soroban_sdk::{contract, contractimpl, Env, Map, Symbol, Val};


    #[contract]
    pub struct TestContract;

    #[contractimpl]
    impl TestContract {}

    fn with_env(run: impl FnOnce(&Env)) {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            treasury::init(&env);
            run(&env);
        });
    }

    #[test]
    fn records_and_retrieves_snapshot() {
        with_env(|env| {
            let ctx: Map<Symbol, Val> = Map::new(env);
            let id = treasury::record_snapshot(env, 1000, 5, TriggerKind::Deposit, ctx);
            let snap = treasury::get_snapshot(env, id).unwrap();
            assert_eq!(snap.id, 1);
            assert_eq!(snap.total_balance, 1000);
            assert!(treasury::verify_snapshot(env, &snap));
        });
    }

    #[test]
    fn recent_snapshots_are_newest_first() {
        with_env(|env| {
            for i in 0..3 {
                let ctx: Map<Symbol, Val> = Map::new(env);
                treasury::record_snapshot(env, 100 + i, 1, TriggerKind::Manual, ctx);
            }
            let ids = treasury::get_recent_snapshots(env, 2);
            assert_eq!(ids.get(0).unwrap(), 3);
            assert_eq!(ids.get(1).unwrap(), 2);
        });
    }

    #[test]
    #[should_panic]
    fn negative_balance_is_rejected() {
        with_env(|env| {
            let ctx: Map<Symbol, Val> = Map::new(env);
            treasury::record_snapshot(env, -1, 0, TriggerKind::Other, ctx);
        });
    }
}
