//! Reentrancy protection tests.

#[cfg(test)]
mod tests {
    use crate::guards::enter_reentrancy_guard;
    use soroban_sdk::{contract, contractimpl, Address, Env};

    #[contract]
    pub struct ReentrantContract;

    #[contractimpl]
    impl ReentrantContract {
        pub fn call_self(env: Env, addr: Address) {
            enter_reentrancy_guard(&env);
            // Attempt reentrant call to same function
            let client = ReentrantContractClient::new(&env, &addr);
            client.call_self(&addr);
        }
    }

    #[test]
    #[should_panic]
    fn test_reentrancy_guard_panics_on_reentry() {
        let env = Env::default();
        let contract_id = env.register_contract(None, ReentrantContract);
        let client = ReentrantContractClient::new(&env, &contract_id);

        client.call_self(&contract_id);
    }
}
