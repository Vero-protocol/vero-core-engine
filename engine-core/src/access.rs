use soroban_sdk::{contracterror, panic_with_error, symbol_short, Address, Env, Map, Symbol};

const KEY_ROLE_MAP: Symbol = symbol_short!("ROLE_MAP");

pub const ROLE_ADMIN: Symbol = symbol_short!("ADMIN");
pub const ROLE_OPERATOR: Symbol = symbol_short!("OP");
pub const ROLE_AUDITOR: Symbol = symbol_short!("AUD");
pub const ROLE_PAUSE_GUARDIAN: Symbol = symbol_short!("PAUSE_GUARD");

#[contracterror]
#[derive(Copy, Clone)]
pub enum AccessError {
    NotAdmin = 1,
    Unauthorized = 2,
}

/// Return the stored role map (Address -> Symbol). If none, return empty Map.
fn load_role_map(env: &Env) -> Map<Address, Symbol> {
    env.storage()
        .instance()
        .get(&KEY_ROLE_MAP)
        .unwrap_or(Map::new(env))
}

/// Check whether `addr` has the given `role`.
pub fn has_role(env: &Env, addr: &Address, role: Symbol) -> bool {
    let role_map: Map<Address, Symbol> = load_role_map(env);
    match role_map.get(addr) {
        Some(r) => r == role,
        None => false,
    }
}

/// Require that `caller` has `ADMIN` role; used to protect role management endpoints.
fn require_admin(env: &Env, caller: &Address) {
    if !has_role(env, caller, ROLE_ADMIN) {
        panic_with_error!(env, AccessError::NotAdmin);
    }
}

/// Grant a role to `target`. Caller must be `ADMIN` and authenticated.
pub fn grant_role(env: &Env, caller: &Address, target: &Address, role: Symbol) {
    caller.require_auth();
    require_admin(env, caller);
    let mut role_map: Map<Address, Symbol> = load_role_map(env);
    role_map.set(target.clone(), role);
    env.storage().instance().set(&KEY_ROLE_MAP, &role_map);
    env.events().publish((symbol_short!("RBAC"), symbol_short!("grant")), target.clone());
}

/// Revoke any role assigned to `target`. Caller must be `ADMIN` and authenticated.
pub fn revoke_role(env: &Env, caller: &Address, target: &Address) {
    caller.require_auth();
    require_admin(env, caller);
    let mut role_map: Map<Address, Symbol> = load_role_map(env);
    // Remove by setting to empty/zero-value; Map in soroban doesn't have remove API, so
    // we set to a sentinel empty symbol (empty string) - treat absence as no role.
    // For simplicity, we set to a symbol that won't match any role.
    let empty = symbol_short!("");
    role_map.set(target.clone(), empty);
    env.storage().instance().set(&KEY_ROLE_MAP, &role_map);
    env.events().publish((symbol_short!("RBAC"), symbol_short!("revoke")), target.clone());
}

/// Require that `addr` has `role`, panicking with `Unauthorized` otherwise.
pub fn require_role(env: &Env, addr: &Address, role: Symbol) {
    if !has_role(env, addr, role) {
        panic_with_error!(env, AccessError::Unauthorized);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, testutils::Address as _};

    #[test]
    fn grant_and_check_role() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);
        // bootstrap admin directly into storage for test
        let mut m: Map<Address, Symbol> = Map::new(&env);
        m.set(admin.clone(), ROLE_ADMIN);
        env.storage().instance().set(&KEY_ROLE_MAP, &m);

        // admin grants OP to alice
        grant_role(&env, &admin, &alice, ROLE_OPERATOR);
        assert!(has_role(&env, &alice, ROLE_OPERATOR));
    }
}
