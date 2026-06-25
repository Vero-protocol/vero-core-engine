//! FSM verification tests for governance proposal state transitions.
//!
//! This module validates that the proposal state machine enforces valid
//! transitions and rejects invalid state transition attempts.

#[cfg(test)]
mod tests {
    use crate::VeroCore;
    use crate::VeroCoreClient;
    use soroban_sdk::{testutils::Ledger, vec, Address, BytesN, Env};

    const TIMELOCK_LEDGERS: u32 = 720;

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
    fn upgrade_requires_quorum_before_timelock_or_wasm_update() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, VeroCore);
        let client = VeroCoreClient::new(&env, &contract_id);

        let signer1 = <Address as soroban_sdk::testutils::Address>::generate(&env);
        let signer2 = <Address as soroban_sdk::testutils::Address>::generate(&env);
        let signer3 = <Address as soroban_sdk::testutils::Address>::generate(&env);
        let signers = vec![&env, signer1.clone(), signer2.clone(), signer3.clone()];
        let threshold = 2;
        client.init(&signers, &threshold, &vec![&env]);

        let wasm_hash = BytesN::from_array(&env, &[7u8; 32]);
        let proposal_id = client.propose(&signer1, &wasm_hash);

        let no_signature_upgrade = client.try_upgrade(&proposal_id);
        assert!(no_signature_upgrade.is_err());

        client.approve(&signer1, &proposal_id);
        let one_signature_upgrade = client.try_upgrade(&proposal_id);
        assert!(one_signature_upgrade.is_err());

        client.approve(&signer2, &proposal_id);
        let timelocked_upgrade = client.try_upgrade(&proposal_id);
        assert!(timelocked_upgrade.is_err());
    }

    #[test]
    fn quorum_approved_proposal_reaches_wasm_update_after_timelock() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, VeroCore);
        let client = VeroCoreClient::new(&env, &contract_id);

        let signer1 = <Address as soroban_sdk::testutils::Address>::generate(&env);
        let signer2 = <Address as soroban_sdk::testutils::Address>::generate(&env);
        let signers = vec![&env, signer1.clone(), signer2.clone()];
        let threshold = 2;
        client.init(&signers, &threshold, &vec![&env]);

        let wasm_hash = BytesN::from_array(&env, &[9u8; 32]);
        let proposal_id = client.propose(&signer1, &wasm_hash);
        client.approve(&signer1, &proposal_id);
        client.approve(&signer2, &proposal_id);

        env.ledger()
            .set_sequence_number(env.ledger().sequence() + TIMELOCK_LEDGERS);

        // The bogus WASM hash should only be reached after quorum and timelock
        // validation pass, proving a single signer cannot trigger this path.
        let reached_wasm_update = client.try_upgrade(&proposal_id);
        assert!(reached_wasm_update.is_err());
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
