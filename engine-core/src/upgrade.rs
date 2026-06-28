//! Governed contract upgrades.

use crate::event_struct::{ACT_UPGRADE, MOD_UPGRADE};
use crate::event_utils::publish_event;
use crate::{governance, types::ProposalState};
use soroban_sdk::{contracterror, panic_with_error, BytesN, Env};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum UpgradeError {
    InvalidWasmHash = 1,
    ProposalNotApproved = 2,
}

/// Upgrade the current contract to `new_wasm_hash` after multi-sig approval.
pub fn upgrade(env: &Env, proposal_id: u64, new_wasm_hash: BytesN<32>) {
    crate::circuit_breaker::assert_closed(env);
    if new_wasm_hash.to_array() == [0u8; 32] {
        panic_with_error!(env, UpgradeError::InvalidWasmHash);
    }

    let proposal = governance::get_proposal(env, proposal_id);
    if proposal.state != ProposalState::Approved {
        panic_with_error!(env, UpgradeError::ProposalNotApproved);
    }
    if proposal.action_hash != new_wasm_hash {
        panic_with_error!(env, UpgradeError::InvalidWasmHash);
    }

    governance::execute(env, proposal_id);
    env.deployer()
        .update_current_contract_wasm(new_wasm_hash.clone());
    publish_event(env, MOD_UPGRADE | ACT_UPGRADE, proposal_id, new_wasm_hash);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{governance, types::Proposal};
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        vec, Address, BytesN, Env,
    };

    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}

    fn proposal(env: &Env, proposer: Address, wasm_hash: BytesN<32>) -> Proposal {
        Proposal {
            id: 7,
            action_hash: wasm_hash,
            proposer,
            approved_by: vec![env],
            state: ProposalState::Executed,
        }
    }

    #[test]
    #[should_panic]
    fn upgrade_fails_without_quorum() {
        let env = Env::default();
        env.mock_all_auths();
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        let hash = BytesN::from_array(&env, &[1u8; 32]);

        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            governance::init(&env, vec![&env, alice.clone(), bob], 2);
            governance::propose(&env, proposal(&env, alice.clone(), hash.clone()));
            governance::approve(&env, &alice, 7);
            upgrade(&env, 7, hash);
        });
    }

    #[test]
    #[should_panic]
    fn upgrade_fails_when_hash_was_not_approved() {
        let env = Env::default();
        env.mock_all_auths();
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        let approved_hash = BytesN::from_array(&env, &[1u8; 32]);
        let unapproved_hash = BytesN::from_array(&env, &[2u8; 32]);

        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            governance::init(&env, vec![&env, alice.clone(), bob.clone()], 2);
            governance::propose(&env, proposal(&env, alice.clone(), approved_hash));
            governance::approve(&env, &alice, 7);
            governance::approve(&env, &bob, 7);
            env.ledger()
                .set_sequence_number(governance::TIMELOCK_LEDGERS + 1);
            upgrade(&env, 7, unapproved_hash);
        });
    }
}
