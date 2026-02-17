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

#[cfg(test)]
mod tests {
    use super::{build_engine, RhaiEngineProfile};
    use rhai::Map;

    #[test]
    fn completion_profile_exposes_safe_subset_only() {
        let engine = build_engine(RhaiEngineProfile::Completion);
        let stat = engine
            .eval::<Map>("fs_stat(\".\")")
            .expect("completion profile should expose fs_stat");
        assert!(stat.contains_key("exists"));

        let proc_call = engine.eval::<Map>("proc_run(\"sh\", [\"-c\", \"echo hi\"], 1000)");
        assert!(proc_call.is_err(), "process APIs must be absent in completion profile");
    }

    #[test]
    fn reconcile_profile_exposes_full_facade_set() {
        let engine = build_engine(RhaiEngineProfile::Reconcile);
        let expr = if cfg!(windows) {
            "proc_run(\"cmd\", [\"/C\", \"echo hi\"], 1000)"
        } else {
            "proc_run(\"sh\", [\"-c\", \"echo hi\"], 1000)"
        };
        let proc_call = engine
            .eval::<Map>(expr)
            .expect("reconcile profile should expose process APIs");
        assert!(proc_call.contains_key("ok"));
    }
}

