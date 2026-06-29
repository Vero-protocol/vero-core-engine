#[cfg(test)]
mod tests {
    use crate::governance::{self, GovError};
    use crate::types::{Proposal, ProposalState};
    use soroban_sdk::{
        testutils::{Address as _, Ledger as _},
        vec, Address, BytesN, Env,
    };

    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}

    fn setup_env(env: &Env) -> Address {
        env.register_contract(None, TestContract)
    }

    fn create_dummy_proposal(env: &Env, proposer: &Address) -> Proposal {
        Proposal {
            id: 1,
            action_hash: BytesN::from_array(env, &[0u8; 32]),
            proposer: proposer.clone(),
            approved_by: vec![env],
            state: ProposalState::Pending,
        }
    }

    fn init_one_of_one(env: &Env, contract_id: &Address, proposer: &Address) {
        env.as_contract(contract_id, || {
            governance::init(env, vec![env, proposer.clone()], 1);
        });
    }

    fn propose_default(env: &Env, contract_id: &Address, proposer: &Address) -> u64 {
        env.as_contract(contract_id, || {
            let prop = create_dummy_proposal(env, proposer);
            governance::propose(env, prop)
        })
    }

    #[test]
    fn test_proposal_initial_state_pending() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = setup_env(&env);
        let proposer = Address::generate(&env);
        env.mock_all_auths();

        init_one_of_one(&env, &contract_id, &proposer);
        let id = propose_default(&env, &contract_id, &proposer);
        env.as_contract(&contract_id, || {
            let (p, _) = governance::load_proposals(&env).get(id).unwrap();
            assert_eq!(p.state, ProposalState::Pending);
        });
    }

    #[test]
    fn test_state_transition_pending_to_approved() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = setup_env(&env);
        let proposer = Address::generate(&env);
        let approver = Address::generate(&env);

        init_one_of_one(&env, &contract_id, &proposer);
        let id = propose_default(&env, &contract_id, &proposer);
        env.as_contract(&contract_id, || governance::approve(&env, &proposer, id));
        env.as_contract(&contract_id, || {
            let (p, unlock) = governance::load_proposals(&env).get(id).unwrap();
            assert_eq!(p.state, ProposalState::Approved);
            assert_eq!(unlock, governance::TIMELOCK_LEDGERS);
        });
    }

    #[test]
    fn test_state_transition_approved_to_executed() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = setup_env(&env);
        let proposer = Address::generate(&env);
        let approver = Address::generate(&env);

        init_one_of_one(&env, &contract_id, &proposer);
        let id = propose_default(&env, &contract_id, &proposer);
        env.as_contract(&contract_id, || governance::approve(&env, &proposer, id));
        env.as_contract(&contract_id, || {
            env.ledger()
                .set_sequence_number(governance::TIMELOCK_LEDGERS + 1);
            governance::execute(&env, id);
            let (p, _) = governance::load_proposals(&env).get(id).unwrap();
            assert_eq!(p.state, ProposalState::Executed);
        });
    }

    #[test]
    #[should_panic]
    fn test_reject_approval_on_approved_proposal() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = setup_env(&env);
        let proposer = Address::generate(&env);
        let approver = Address::generate(&env);
        let signer2 = Address::generate(&env);

        env.as_contract(&contract_id, || {
            governance::init(&env, vec![&env, proposer.clone(), signer2.clone()], 1);
        });
        let id = propose_default(&env, &contract_id, &proposer);
        env.as_contract(&contract_id, || governance::approve(&env, &proposer, id));
        env.as_contract(&contract_id, || governance::approve(&env, &signer2, id));
    }

    #[test]
    #[should_panic]
    fn test_reject_execution_of_pending_proposal() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = setup_env(&env);
        let proposer = Address::generate(&env);
        env.mock_all_auths();

        init_one_of_one(&env, &contract_id, &proposer);
        let id = propose_default(&env, &contract_id, &proposer);
        env.as_contract(&contract_id, || governance::execute(&env, id));
    }

    #[test]
    #[should_panic]
    fn test_reject_double_execution() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = setup_env(&env);
        let proposer = Address::generate(&env);
        let approver = Address::generate(&env);

        init_one_of_one(&env, &contract_id, &proposer);
        let id = propose_default(&env, &contract_id, &proposer);
        env.as_contract(&contract_id, || governance::approve(&env, &proposer, id));
        env.as_contract(&contract_id, || {
            env.ledger()
                .set_sequence_number(governance::TIMELOCK_LEDGERS + 1);
            governance::execute(&env, id);
            governance::execute(&env, id);
        });
    }

    #[test]
    #[should_panic]
    fn test_reject_approval_of_executed_proposal() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = setup_env(&env);
        let proposer = Address::generate(&env);
        let approver = Address::generate(&env);
        let signer2 = Address::generate(&env);

        env.as_contract(&contract_id, || {
            governance::init(&env, vec![&env, proposer.clone(), signer2.clone()], 1);
        });
        let id = propose_default(&env, &contract_id, &proposer);
        env.as_contract(&contract_id, || governance::approve(&env, &proposer, id));
        env.as_contract(&contract_id, || {
            env.ledger()
                .set_sequence_number(governance::TIMELOCK_LEDGERS + 1);
            governance::execute(&env, id);
        });
        env.as_contract(&contract_id, || governance::approve(&env, &signer2, id));
    }

    #[test]
    fn test_full_proposal_lifecycle() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = setup_env(&env);
        let proposer = Address::generate(&env);
        let approver = Address::generate(&env);

        init_one_of_one(&env, &contract_id, &proposer);
        let id = propose_default(&env, &contract_id, &proposer);
        env.as_contract(&contract_id, || {
            assert_eq!(
                governance::get_proposal(&env, id).state,
                ProposalState::Pending
            );
        });
        env.as_contract(&contract_id, || governance::approve(&env, &proposer, id));
        env.as_contract(&contract_id, || {
            assert_eq!(
                governance::get_proposal(&env, id).state,
                ProposalState::Approved
            );
            env.ledger()
                .set_sequence_number(governance::TIMELOCK_LEDGERS + 1);
            governance::execute(&env, id);
            assert_eq!(
                governance::get_proposal(&env, id).state,
                ProposalState::Executed
            );
        });
    }

    #[test]
    fn test_invalid_transition_error_code() {
        assert_eq!(GovError::InvalidStateTransition as u32, 5);
    }

    #[test]
    #[should_panic]
    fn test_duplicate_approval_detection() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = setup_env(&env);
        let proposer = Address::generate(&env);
        let signer2 = Address::generate(&env);

        env.as_contract(&contract_id, || {
            governance::init(&env, vec![&env, proposer.clone(), signer2], 2);
        });
        let id = propose_default(&env, &contract_id, &proposer);
        env.as_contract(&contract_id, || {
            governance::approve(&env, &proposer, id);
            governance::approve(&env, &proposer, id);
        });
    }
}

/// State Transition Matrix (for documentation)
///
/// | Current State | Operation | Target State | Allowed | Error |
/// |---|---|---|---|---|
/// | Pending | approve (< threshold) | Pending | Yes | — |
/// | Pending | approve (>= threshold) | Approved | Yes | — |
/// | Pending | execute | — | No | InvalidStateTransition |
/// | Approved | approve | — | No | InvalidStateTransition |
/// | Approved | execute (timelock OK) | Executed | Yes | — |
/// | Approved | execute (timelock active) | — | No | TimelockActive |
/// | Executed | approve | — | No | InvalidStateTransition |
/// | Executed | execute | — | No | InvalidStateTransition |
#[allow(dead_code)]
pub struct StateTransitionMatrix;
}
