//! Reentrancy protection tests.

#[cfg(test)]
mod tests {

    use crate::guards::{enter_reentrancy_guard, exit_reentrancy_guard};
    use soroban_sdk::{contract, contractimpl, Env};

    use crate::guards::enter_reentrancy_guard;
    use soroban_sdk::{contract, contractimpl, Address, Env};


    #[contract]
    pub struct TestContract;

    #[contractimpl]
    impl TestContract {}

    #[test]
    #[should_panic]
    fn guard_panics_when_entered_twice() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            enter_reentrancy_guard(&env);
            enter_reentrancy_guard(&env);
        });
    }

    #[test]
    fn guard_can_be_reentered_after_exit() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            enter_reentrancy_guard(&env);
            exit_reentrancy_guard(&env);
            enter_reentrancy_guard(&env);
            exit_reentrancy_guard(&env);
        });
    }
}
