//! Governance tests — state machine transitions and anti-Sybil stake gate.

#[cfg(test)]
mod tests {
    use crate::governance;
    use crate::types::{Proposal, ProposalState};
    use soroban_sdk::{
        contract, contractimpl,
        testutils::{Address as _, Events, Ledger},
        vec, Address, BytesN, Env,
    };

    // ── minimal stub contract ────────────────────────────────────────────────

    #[contract]
    struct GovContract;

    #[contractimpl]
    impl GovContract {}

    // ── helpers ──────────────────────────────────────────────────────────────

    fn register_contract(env: &Env) -> Address {
        env.register_contract(None, GovContract)
    }

    fn register_token(env: &Env) -> Address {
        env.register_stellar_asset_contract_v2(Address::generate(env))
            .address()
    }

    fn fund(env: &Env, token: &Address, to: &Address, amount: i128) {
        soroban_sdk::token::StellarAssetClient::new(env, token)
            .mock_all_auths()
            .mint(to, &amount);
    }

    fn dummy_hash(env: &Env) -> BytesN<32> {
        BytesN::from_array(env, &[0u8; 32])
    }

    fn make_proposal(env: &Env, id: u64, proposer: &Address) -> Proposal {
        Proposal {
            id,
            action_hash:  dummy_hash(env),
            proposer:     proposer.clone(),
            approved_by:  vec![env],
            state:        ProposalState::Pending,
        }
    }

    /// Read a proposal back from persistent storage using the hex-encoded key.
    fn get_proposal(env: &Env, id: u64) -> (Proposal, u32) {
        let key = Symbol::new(env, &format!("P{:x}", id));
        env.storage().persistent().get(&key).unwrap()
    }

    fn init_one(env: &Env, signer: &Address, min_stake: i128) -> (Address, Address) {
        let cid   = register_contract(env);
        let token = register_token(env);
        env.as_contract(&cid, || {
            governance::init(env, vec![env, signer.clone()], 1, token.clone(), min_stake);
        });
        (cid, token)
    }

    fn init_two(env: &Env, a: &Address, b: &Address) -> Address {
        let cid = register_contract(env);
        env.as_contract(&cid, || {
            governance::init(
                env,
                vec![env, a.clone(), b.clone()],
                2,
                register_token(env),
                0,
            );
        });
        cid
    }

    // ── state transitions ─────────────────────────────────────────────────

    #[test]
    fn test_state_transition_approved_to_executed() {
        let env = Env::default();
        env.mock_all_auths();
        let s1 = Address::generate(&env);
        let (cid, _) = init_one(&env, &s1, 0);

        env.as_contract(&cid, || {
            let id = governance::propose(&env, make_proposal(&env, 1, &s1));
            governance::approve(&env, &s1, id);
            env.ledger().with_mut(|l| l.sequence_number += 721);
            let executed_prop = governance::execute(&env, id);
            assert_eq!(executed_prop.state, ProposalState::Executed);
        });
    }

    #[test]
    fn test_auto_execute_on_approve_after_timelock() {
        let env = Env::default();
        env.mock_all_auths();
        let s1 = Address::generate(&env);
        let (cid, _) = init_one(&env, &s1, 0);

        env.as_contract(&cid, || {
            let id = governance::propose(&env, make_proposal(&env, 2, &s1));
            env.ledger().with_mut(|l| l.sequence_number += 2000);
            governance::approve(&env, &s1, id);

            let (prop, _) = get_proposal(&env, id);
            assert_eq!(prop.state, ProposalState::Executed);
        });
    }

    // ── anti-Sybil stake gate ─────────────────────────────────────────────

    #[test]
    fn test_approve_passes_with_sufficient_stake() {
        let env = Env::default();
        env.mock_all_auths();
        let signer = Address::generate(&env);
        let (cid, token) = init_one(&env, &signer, 1_000);
        fund(&env, &token, &signer, 1_000);

        env.as_contract(&cid, || {
            let id = governance::propose(&env, make_proposal(&env, 1, &signer));
            governance::approve(&env, &signer, id);
            let (prop, _) = get_proposal(&env, id);
            assert_eq!(prop.state, ProposalState::Approved);
        });
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #7)")]
    fn test_approve_fails_with_insufficient_stake() {
        let env = Env::default();
        env.mock_all_auths();
        let signer = Address::generate(&env);
        let (cid, token) = init_one(&env, &signer, 1_000);
        fund(&env, &token, &signer, 999);

        env.as_contract(&cid, || {
            let id = governance::propose(&env, make_proposal(&env, 1, &signer));
            governance::approve(&env, &signer, id);
        });
    }

    #[test]
    fn test_stake_gate_disabled_when_min_stake_zero() {
        let env = Env::default();
        env.mock_all_auths();
        let signer = Address::generate(&env);
        let (cid, _) = init_one(&env, &signer, 0);

        env.as_contract(&cid, || {
            let id = governance::propose(&env, make_proposal(&env, 1, &signer));
            governance::approve(&env, &signer, id);
            let (prop, _) = get_proposal(&env, id);
            assert_eq!(prop.state, ProposalState::Approved);
        });
    }

    // ── full lifecycle ────────────────────────────────────────────────────

    #[test]
    fn test_full_lifecycle() {
        let env = Env::default();
        env.mock_all_auths();
        let a = Address::generate(&env);
        let b = Address::generate(&env);
        let cid = init_two(&env, &a, &b);

        env.as_contract(&cid, || {
            let id = governance::propose(&env, make_proposal(&env, 1, &a));

            governance::approve(&env, &a, id);
            assert_eq!(get_proposal(&env, id).0.state, ProposalState::Pending);

            governance::approve(&env, &b, id);
            assert_eq!(get_proposal(&env, id).0.state, ProposalState::Approved);

            env.ledger().with_mut(|l| l.sequence_number += 721);
            let prop = governance::execute(&env, id);
            assert_eq!(prop.state, ProposalState::Executed);
        });
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #5)")]
    fn test_execute_pending_proposal_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let a = Address::generate(&env);
        let cid = init_two(&env, &a, &Address::generate(&env));

        env.as_contract(&cid, || {
            let id = governance::propose(&env, make_proposal(&env, 1, &a));
            governance::execute(&env, id);
        });
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #4)")]
    fn test_execute_before_timelock_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let signer = Address::generate(&env);
        let (cid, _) = init_one(&env, &signer, 0);

        env.as_contract(&cid, || {
            let id = governance::propose(&env, make_proposal(&env, 1, &signer));
            governance::approve(&env, &signer, id);
            governance::execute(&env, id);
        });
    }

    #[test]
    #[should_panic]
    fn test_duplicate_approval_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let a = Address::generate(&env);
        let cid = init_two(&env, &a, &Address::generate(&env));

        env.as_contract(&cid, || {
            let id = governance::propose(&env, make_proposal(&env, 1, &a));
            governance::approve(&env, &a, id);
            governance::approve(&env, &a, id);
        });
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #1)")]
    fn test_non_signer_cannot_approve() {
        let env = Env::default();
        env.mock_all_auths();
        let a = Address::generate(&env);
        let outsider = Address::generate(&env);
        let cid = init_two(&env, &a, &Address::generate(&env));

        env.as_contract(&cid, || {
            let id = governance::propose(&env, make_proposal(&env, 1, &a));
            governance::approve(&env, &outsider, id);
        });
    }

    // ── cancel / revert ───────────────────────────────────────────────────

    #[test]
    fn test_cancel_pending_proposal() {
        let env = Env::default();
        env.mock_all_auths();
        let s1 = Address::generate(&env);
        let (cid, _) = init_one(&env, &s1, 0);

        env.as_contract(&cid, || {
            let id = governance::propose(&env, make_proposal(&env, 1, &s1));
            let prop = governance::cancel(&env, &s1, id);
            assert_eq!(prop.state, ProposalState::Cancelled);

            // The stored proposal is the cancelled (terminal) one.
            let stored = env
                .storage()
                .instance()
                .get::<_, soroban_sdk::Map<u64, (Proposal, u32)>>(
                    &soroban_sdk::symbol_short!("PROPS"),
                )
                .unwrap()
                .get(id)
                .unwrap()
                .0
                .state;
            assert_eq!(stored, ProposalState::Cancelled);
        });
    }

    #[test]
    fn test_cancel_before_threshold_by_other_signer() {
        let env = Env::default();
        env.mock_all_auths();
        let a = Address::generate(&env);
        let b = Address::generate(&env);
        let cid = init_two(&env, &a, &b); // threshold = 2

        env.as_contract(&cid, || {
            let id = governance::propose(&env, make_proposal(&env, 1, &a));
            governance::approve(&env, &a, id); // still Pending (1 of 2)
            let prop = governance::cancel(&env, &b, id);
            assert_eq!(prop.state, ProposalState::Cancelled);
        });
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #6)")]
    fn test_cannot_cancel_executed_proposal() {
        let env = Env::default();
        env.mock_all_auths();
        let s1 = Address::generate(&env);
        let (cid, _) = init_one(&env, &s1, 0);

        env.as_contract(&cid, || {
            let id = governance::propose(&env, make_proposal(&env, 1, &s1));
            governance::approve(&env, &s1, id); // → Approved
            env.ledger().with_mut(|l| l.sequence_number += 721);
            governance::execute(&env, id); // → Executed
            governance::cancel(&env, &s1, id); // AlreadyExecuted = 6
        });
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #5)")]
    fn test_cannot_cancel_twice() {
        let env = Env::default();
        env.mock_all_auths();
        let a = Address::generate(&env);
        let b = Address::generate(&env);
        let cid = init_two(&env, &a, &b);

        env.as_contract(&cid, || {
            let id = governance::propose(&env, make_proposal(&env, 1, &a));
            governance::cancel(&env, &a, id); // → Cancelled
            governance::cancel(&env, &b, id); // InvalidStateTransition = 5
        });
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #1)")]
    fn test_non_signer_cannot_cancel() {
        let env = Env::default();
        env.mock_all_auths();
        let a = Address::generate(&env);
        let outsider = Address::generate(&env);
        let cid = init_two(&env, &a, &Address::generate(&env));

        env.as_contract(&cid, || {
            let id = governance::propose(&env, make_proposal(&env, 1, &a));
            governance::cancel(&env, &outsider, id); // NotASigner = 1
        });
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #5)")]
    fn test_cannot_approve_cancelled_proposal() {
        let env = Env::default();
        env.mock_all_auths();
        let a = Address::generate(&env);
        let b = Address::generate(&env);
        let cid = init_two(&env, &a, &b);

        env.as_contract(&cid, || {
            let id = governance::propose(&env, make_proposal(&env, 1, &a));
            governance::cancel(&env, &a, id); // → Cancelled
            governance::approve(&env, &b, id); // InvalidStateTransition = 5
        });
    }
}
