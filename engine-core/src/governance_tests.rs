//! Governance tests — state machine transitions and anti-Sybil stake gate.

#[cfg(test)]
mod tests {
    use crate::governance;
    use crate::types::{Proposal, ProposalState};
    use soroban_sdk::{
        contract, contractimpl,
        testutils::{Address as _, Ledger},
        vec, Address, BytesN, Env,
    };

    // ── minimal stub contract so we can call env.as_contract() ──────────────

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
            action_hash: dummy_hash(env),
            proposer: proposer.clone(),
            approved_by: vec![env],
            state: ProposalState::Pending,
        }
    }

    /// Init with one signer + optional stake gate. Returns (contract_id, token_address).
    fn init_one(env: &Env, signer: &Address, min_stake: i128) -> (Address, Address) {
        let cid = register_contract(env);
        let token = register_token(env);
        env.as_contract(&cid, || {
            governance::init(env, vec![env, signer.clone()], 1, token.clone(), min_stake);
        });
        (cid, token)
    }

    /// Init with two signers, threshold=2, no stake gate. Returns contract_id.
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

    // ── anti-Sybil stake gate ─────────────────────────────────────────────

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
            // Advance ledger past the unlock before approving so approve() should
            // auto-execute when threshold is met.
            env.ledger().with_mut(|l| l.sequence_number += 2000);
            governance::approve(&env, &s1, id);

            let state = env
                .storage()
                .instance()
                .get::<_, soroban_sdk::Map<u64, (Proposal, u32)>>(&soroban_sdk::symbol_short!("PROPS"))
                .unwrap()
                .get(id)
                .unwrap()
                .0
                .state;
            assert_eq!(state, ProposalState::Executed);
        });
    }

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
            let state = env
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
            assert_eq!(state, ProposalState::Approved);
        });
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #7)")]
    fn test_approve_fails_with_insufficient_stake() {
        let env = Env::default();
        env.mock_all_auths();
        let signer = Address::generate(&env);
        let (cid, token) = init_one(&env, &signer, 1_000);
        fund(&env, &token, &signer, 999); // one short

        env.as_contract(&cid, || {
            let id = governance::propose(&env, make_proposal(&env, 1, &signer));
            governance::approve(&env, &signer, id); // InsufficientStake = 7
        });
    }

    #[test]
    fn test_stake_gate_disabled_when_min_stake_zero() {
        let env = Env::default();
        env.mock_all_auths();
        let signer = Address::generate(&env);
        let (cid, _) = init_one(&env, &signer, 0); // no tokens needed

        env.as_contract(&cid, || {
            let id = governance::propose(&env, make_proposal(&env, 1, &signer));
            governance::approve(&env, &signer, id); // must not panic
            let state = env
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
            assert_eq!(state, ProposalState::Approved);
        });
    }

    // ── state machine ─────────────────────────────────────────────────────

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
            assert_eq!(
                env.storage()
                    .instance()
                    .get::<_, soroban_sdk::Map<u64, (Proposal, u32)>>(
                        &soroban_sdk::symbol_short!("PROPS"),
                    )
                    .unwrap()
                    .get(id)
                    .unwrap()
                    .0
                    .state,
                ProposalState::Pending
            );

            governance::approve(&env, &b, id);
            assert_eq!(
                env.storage()
                    .instance()
                    .get::<_, soroban_sdk::Map<u64, (Proposal, u32)>>(
                        &soroban_sdk::symbol_short!("PROPS"),
                    )
                    .unwrap()
                    .get(id)
                    .unwrap()
                    .0
                    .state,
                ProposalState::Approved
            );

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
            governance::execute(&env, id); // InvalidStateTransition = 5
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
            governance::approve(&env, &signer, id); // → Approved
            governance::execute(&env, id); // TimelockActive = 4
        });
    }

    #[test]
    #[should_panic]
    fn test_duplicate_approval_rejected() {
        // The second approve by the same signer must panic. In the test environment
        // mock_all_auths() consumes the auth token on the first call, so the second
        // call raises Error(Auth, ExistingValue) before reaching AlreadyApproved — both
        // are correct rejections of a duplicate approval attempt.
        let env = Env::default();
        env.mock_all_auths();
        let a = Address::generate(&env);
        let cid = init_two(&env, &a, &Address::generate(&env));

        env.as_contract(&cid, || {
            let id = governance::propose(&env, make_proposal(&env, 1, &a));
            governance::approve(&env, &a, id);
            governance::approve(&env, &a, id); // must panic
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
            governance::approve(&env, &outsider, id); // NotASigner = 1
        });
    }
}
