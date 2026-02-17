use crate::rhai_engine::RhaiEngineProfile;
use rhai::Engine;

mod fs;
mod http;
mod platform;
mod process;
mod soap;

pub fn register(engine: &mut Engine, profile: RhaiEngineProfile) {
    platform::register(engine);

    match profile {
        RhaiEngineProfile::Completion => {
            fs::register_read_only(engine);
        }
        RhaiEngineProfile::Reconcile => {
            fs::register_full(engine);
            process::register(engine);
            http::register(engine);
            soap::register(engine);
        }
    }
}
