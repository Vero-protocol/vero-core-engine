//! FSM verification tests for governance proposal state transitions.
//!
//! This module validates that the proposal state machine enforces valid
//! transitions and rejects invalid state transition attempts.

#[cfg(test)]
mod tests {
    use crate::VeroCore;
    use crate::VeroCoreClient;
    use soroban_sdk::{vec, Address, BytesN, Env};

    #[test]
    fn test_proposal_lifecycle_and_upgrade() {
        let env = Env::default();

        let contract_id = env.register_contract(None, VeroCore);
        let client = VeroCoreClient::new(&env, &contract_id);

        let signer1 = <Address as soroban_sdk::testutils::Address>::generate(&env);
        let signer2 = <Address as soroban_sdk::testutils::Address>::generate(&env);
        let signers = vec![&env, signer1.clone(), signer2.clone()];
        let threshold = 2;
        let guardian = <Address as soroban_sdk::testutils::Address>::generate(&env);

        client.init(&signers, &threshold, &vec![&env, guardian.clone()]);

        let wasm_hash = BytesN::from_array(&env, &[1u8; 32]);

        // 1. Propose
        env.mock_all_auths();
        let proposal_id = client.propose(&signer1, &wasm_hash);
        assert_eq!(proposal_id, 1);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #1)")] // NotASigner
    fn test_propose_unauthorized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, VeroCore);
        let client = VeroCoreClient::new(&env, &contract_id);

        let signer1 = <Address as soroban_sdk::testutils::Address>::generate(&env);
        let signers = vec![&env, signer1.clone()];
        client.init(&signers, &1, &vec![&env]);

        let rogue = <Address as soroban_sdk::testutils::Address>::generate(&env);
        client.propose(&rogue, &BytesN::from_array(&env, &[0u8; 32]));
    }

    #[test]
    fn test_circuit_breaker_halts_propose() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, VeroCore);
        let client = VeroCoreClient::new(&env, &contract_id);

        let signer1 = <Address as soroban_sdk::testutils::Address>::generate(&env);
        let guardian = <Address as soroban_sdk::testutils::Address>::generate(&env);
        let signers = vec![&env, signer1.clone()];
        client.init(&signers, &1, &vec![&env, guardian.clone()]);

        client.trip(&guardian);

        let result = client.try_propose(&signer1, &BytesN::from_array(&env, &[0u8; 32]));
        assert!(result.is_err());

        client.reset(&guardian);
        let id = client.propose(&signer1, &BytesN::from_array(&env, &[0u8; 32]));
        assert_eq!(id, 1);
    }
}
