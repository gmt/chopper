use crate::path_mutation::{self, SinglePathOpKind};
use crate::rhai_facade_validation::{array_to_strings, ensure_no_nul, facade_error, RhaiResult};
use rhai::{Array, Dynamic, Engine, ImmutableString};

pub fn register(engine: &mut Engine) {
    engine.register_fn("pathlist_split", pathlist_split);
    engine.register_fn("pathlist_join", pathlist_join);
    engine.register_fn("pathlist_remove_all", pathlist_remove_all);
    engine.register_fn("pathlist_remove_one", pathlist_remove_one);
    engine.register_fn("pathlist_append_all", pathlist_append_all);
    engine.register_fn("pathlist_append_one", pathlist_append_one);
    engine.register_fn("pathlist_prepend_all", pathlist_prepend_all);
    engine.register_fn("pathlist_prepend_one", pathlist_prepend_one);
}

fn pathlist_split(list: &str) -> RhaiResult<Array> {
    ensure_no_nul("list", list)?;
    Ok(path_mutation::split_colon_list(list)
        .into_iter()
        .map(|value| Dynamic::from(ImmutableString::from(value)))
        .collect())
}

fn pathlist_join(components: Array) -> RhaiResult<String> {
    let components = array_to_strings("components", &components)?;
    Ok(path_mutation::join_colon_list(&components))
}

fn pathlist_remove_all(list: &str, regex: &str) -> RhaiResult<String> {
    apply_op(
        list,
        SinglePathOpKind::RemoveAll,
        regex,
        "pathlist_remove_all",
    )
}

fn pathlist_remove_one(list: &str, regex: &str) -> RhaiResult<String> {
    apply_op(
        list,
        SinglePathOpKind::RemoveOne,
        regex,
        "pathlist_remove_one",
    )
}

fn pathlist_append_all(list: &str, path: &str) -> RhaiResult<String> {
    apply_op(
        list,
        SinglePathOpKind::AppendAll,
        path,
        "pathlist_append_all",
    )
}

fn pathlist_append_one(list: &str, path: &str) -> RhaiResult<String> {
    apply_op(
        list,
        SinglePathOpKind::AppendOne,
        path,
        "pathlist_append_one",
    )
}

fn pathlist_prepend_all(list: &str, path: &str) -> RhaiResult<String> {
    apply_op(
        list,
        SinglePathOpKind::PrependAll,
        path,
        "pathlist_prepend_all",
    )
}

fn pathlist_prepend_one(list: &str, path: &str) -> RhaiResult<String> {
    apply_op(
        list,
        SinglePathOpKind::PrependOne,
        path,
        "pathlist_prepend_one",
    )
}

fn apply_op(
    list: &str,
    kind: SinglePathOpKind,
    operand: &str,
    context: &str,
) -> RhaiResult<String> {
    path_mutation::apply_single_colon_list_op(list, kind, operand, context)
        .map_err(|err| facade_error(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{pathlist_join, pathlist_remove_all, pathlist_split};
    use rhai::{Array, Dynamic, ImmutableString};

    #[test]
    fn split_and_join_round_trip() {
        let values = pathlist_split(":/usr/bin:/bin").expect("split");
        assert_eq!(values.len(), 3);

        let joined = pathlist_join(values).expect("join");
        assert_eq!(joined, ":/usr/bin:/bin");
    }

    #[test]
    fn join_rejects_non_string_values() {
        let values: Array = vec![Dynamic::from(1_i64)];
        assert!(pathlist_join(values).is_err());
    }

    #[test]
    fn remove_all_reports_invalid_regex() {
        let err = pathlist_remove_all("/bin:/usr/bin", "(").expect_err("invalid regex");
        assert!(
            err.to_string()
                .contains("pathlist_remove_all contains invalid regex `(`"),
            "{err}"
        );
    }

    #[test]
    fn split_preserves_empty_components() {
        let values = pathlist_split("a::b").expect("split");
        let strings: Vec<_> = values
            .into_iter()
            .map(|value| value.cast::<ImmutableString>().to_string())
            .collect();
        assert_eq!(strings, vec!["a", "", "b"]);
    }
}
