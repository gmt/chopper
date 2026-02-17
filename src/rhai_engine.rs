use rhai::Engine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RhaiEngineProfile {
    Reconcile,
    Completion,
}

pub fn build_engine(profile: RhaiEngineProfile) -> Engine {
    let mut engine = Engine::new();
    crate::rhai_facade::register(&mut engine, profile);
    engine
}

