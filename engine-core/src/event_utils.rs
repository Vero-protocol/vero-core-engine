use soroban_sdk::{Env, Symbol, BytesN, Map, Val};
use crate::event_struct::Event;

pub fn publish_event(env: &Env, event_type: BytesN<32>, action: BytesN<32>, payload: Map<Symbol, Val>) {
    let ev = Event {
        event_type,
        action,
        payload,
    };
    // Emit the event with a generic identifier.
    env.events().publish((Symbol::short("EVENT"), Symbol::short("LOG")), ev);
}
