use soroban_sdk::{contracterror, panic_with_error, symbol_short, vec, Address, BytesN, Env, Symbol, Vec};
use crate::event_struct::{MOD_CORE, ACT_INIT};
use crate::event_utils::publish_event;

const KEY_INIT:     Symbol = symbol_short!("C_INIT");
const KEY_ADMIN:    Symbol = symbol_short!("C_ADMIN");
const KEY_OPERATRS: Symbol = symbol_short!("C_OPERS");
const KEY_AUDITORS: Symbol = symbol_short!("C_AUDIT");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AccessError {
    Unauthorized      = 1,
    AlreadyInit       = 2,
    NotInitialized    = 3,
    InvalidAdmin      = 4,
    InvalidOperator   = 5,
    InvalidAuditor    = 6,
}

pub const ROLE_ADMIN:    u32 = 0;
pub const ROLE_OPERATOR: u32 = 1;
pub const ROLE_AUDITOR:  u32 = 2;

pub fn initialize(env: &Env, admin: &Address, operators: Vec<Address>, auditors: Vec<Address>) {
    if is_initialized(env) {
        panic_with_error!(env, AccessError::AlreadyInit);
    }

    admin.require_auth();

    env.storage().instance().set(&KEY_INIT, &true);
    env.storage().instance().set(&KEY_ADMIN, admin);

    let mut op_vec: Vec<Address> = vec![env];
    for o in operators.iter() {
        op_vec.push_back(o);
    }
    env.storage().instance().set(&KEY_OPERATRS, &op_vec);

    let mut au_vec: Vec<Address> = vec![env];
    for a in auditors.iter() {
        au_vec.push_back(a);
    }
    env.storage().instance().set(&KEY_AUDITORS, &au_vec);

    publish_event(
        env,
        MOD_CORE | ACT_INIT,
        0,
        BytesN::from_array(env, &[0u8; 32]),
    );
}

pub fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&KEY_INIT)
}

pub fn require_initialized(env: &Env) {
    if !is_initialized(env) {
        panic_with_error!(env, AccessError::NotInitialized);
    }
}

pub fn require_role(env: &Env, caller: &Address, role: u32) {
    caller.require_auth();
    require_initialized(env);

    let ok = match role {
        ROLE_ADMIN => {
            let admin: Address = env.storage().instance().get(&KEY_ADMIN)
                .unwrap_or_else(|| panic_with_error!(env, AccessError::NotInitialized));
            admin == caller.clone()
        }
        ROLE_OPERATOR => {
            let operators: Vec<Address> = env.storage().instance().get(&KEY_OPERATRS)
                .unwrap_or_else(|| vec![env]);
            operators.contains(caller)
        }
        ROLE_AUDITOR => {
            let auditors: Vec<Address> = env.storage().instance().get(&KEY_AUDITORS)
                .unwrap_or_else(|| vec![env]);
            auditors.contains(caller)
        }
        _ => false,
    };

    if !ok {
        panic_with_error!(env, AccessError::Unauthorized);
    }
}

pub fn get_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&KEY_ADMIN)
}

pub fn get_operators(env: &Env) -> Vec<Address> {
    env.storage().instance().get(&KEY_OPERATRS).unwrap_or(vec![env])
}

pub fn get_auditors(env: &Env) -> Vec<Address> {
    env.storage().instance().get(&KEY_AUDITORS).unwrap_or(vec![env])
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, vec, Env};

    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}

    fn setup(env: &Env) -> (Address, Address, Vec<Address>, Vec<Address>) {
        let admin = Address::generate(env);
        let op1 = Address::generate(env);
        let au1 = Address::generate(env);
        let operators = vec![env, op1.clone()];
        let auditors = vec![env, au1.clone()];
        (admin, op1, operators, auditors)
    }

    #[test]
    fn initialize_sets_admin() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TestContract);
        let (admin, _, operators, auditors) = setup(&env);
        env.as_contract(&contract_id, || {
            initialize(&env, &admin, operators, auditors);
            assert!(is_initialized(&env));
            assert_eq!(get_admin(&env).unwrap(), admin);
        });
    }

    #[test]
    #[should_panic]
    fn double_init_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TestContract);
        let (admin, _, operators, auditors) = setup(&env);
        env.as_contract(&contract_id, || {
            initialize(&env, &admin, operators.clone(), auditors.clone());
            initialize(&env, &admin, operators, auditors);
        });
    }

    #[test]
    fn admin_role_authorized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TestContract);
        let (admin, _, operators, auditors) = setup(&env);
        env.as_contract(&contract_id, || {
            initialize(&env, &admin, operators, auditors);
            require_role(&env, &admin, ROLE_ADMIN);
        });
    }

    #[test]
    fn operator_role_authorized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TestContract);
        let (admin, op1, operators, auditors) = setup(&env);
        env.as_contract(&contract_id, || {
            initialize(&env, &admin, operators, auditors);
            require_role(&env, &op1, ROLE_OPERATOR);
        });
    }

    #[test]
    #[should_panic]
    fn operator_not_admin() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TestContract);
        let (admin, op1, operators, auditors) = setup(&env);
        let rogue = Address::generate(&env);
        env.as_contract(&contract_id, || {
            initialize(&env, &admin, operators, auditors);
            require_role(&env, &rogue, ROLE_ADMIN);
        });
    }

    #[test]
    fn auditor_role_authorized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TestContract);
        let (admin, _, operators, auditors) = setup(&env);
        let au1 = auditors.get(0).unwrap();
        env.as_contract(&contract_id, || {
            initialize(&env, &admin, operators, auditors);
            require_role(&env, &au1, ROLE_AUDITOR);
        });
    }
}
