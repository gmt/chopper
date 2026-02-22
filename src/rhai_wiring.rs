use anyhow::{anyhow, Context, Result};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RhaiMethodKind {
    Reconcile,
    Bashcomp,
}

pub(crate) fn shared_rhai_path_for_alias_doc(alias_doc_path: &Path) -> PathBuf {
    let canonical = fs_err::canonicalize(alias_doc_path).unwrap_or_else(|_| alias_doc_path.into());
    let parent = canonical.parent().unwrap_or_else(|| Path::new("."));
    let stem = canonical
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            alias_doc_path
                .file_stem()
                .and_then(|value| value.to_str())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or("alias");
    parent.join(format!("{stem}.rhai"))
}

pub(crate) fn read_compatible_methods(path: &Path) -> Result<Vec<String>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let source =
        fs_err::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(parse_compatible_methods(&source))
}

pub(crate) fn parse_compatible_methods(source: &str) -> Vec<String> {
    let mut names = BTreeSet::new();
    for line in source.lines() {
        if let Some(name) = parse_method_decl_line(line) {
            names.insert(name.to_string());
        }
    }
    names.into_iter().collect()
}

pub(crate) fn ensure_method_exists(path: &Path, method_name: &str, kind: RhaiMethodKind) -> Result<()> {
    let method_name = normalize_method_name(method_name)?;
    if let Some(parent) = path.parent() {
        fs_err::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    if !path.exists() {
        fs_err::write(path, method_template(method_name, kind))
            .with_context(|| format!("failed to write {}", path.display()))?;
        return Ok(());
    }
    let source =
        fs_err::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let methods = parse_compatible_methods(&source);
    if methods.iter().any(|candidate| candidate == method_name) {
        return Ok(());
    }

    let mut updated = source;
    if !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push('\n');
    updated.push_str(&method_template(method_name, kind));
    fs_err::write(path, updated).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub(crate) fn rename_method_or_create(
    path: &Path,
    old_name: Option<&str>,
    new_name: &str,
    kind: RhaiMethodKind,
) -> Result<()> {
    let new_name = normalize_method_name(new_name)?;
    if let Some(parent) = path.parent() {
        fs_err::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    if !path.exists() {
        fs_err::write(path, method_template(new_name, kind))
            .with_context(|| format!("failed to write {}", path.display()))?;
        return Ok(());
    }

    let source =
        fs_err::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let old_name = old_name.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });

    if let Some(old_name) = old_name {
        if old_name == new_name {
            ensure_method_exists(path, new_name, kind)?;
            return Ok(());
        }
        if let Some(renamed) = rename_method_declaration_in_source(&source, old_name, new_name) {
            fs_err::write(path, renamed)
                .with_context(|| format!("failed to write {}", path.display()))?;
            return Ok(());
        }
    }

    ensure_method_exists(path, new_name, kind)
}

pub(crate) fn sync_method_after_edit(
    before_methods: &[String],
    after_methods: &[String],
    configured_method: Option<&str>,
) -> Option<String> {
    let configured = configured_method
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if after_methods.iter().any(|name| name == configured) {
        return Some(configured.to_string());
    }

    let before_set: BTreeSet<&str> = before_methods.iter().map(String::as_str).collect();
    let after_set: BTreeSet<&str> = after_methods.iter().map(String::as_str).collect();
    let disappeared: Vec<&str> = before_set.difference(&after_set).copied().collect();
    let appeared: Vec<&str> = after_set.difference(&before_set).copied().collect();
    if disappeared.len() == 1 && appeared.len() == 1 && disappeared[0] == configured {
        return Some(appeared[0].to_string());
    }
    None
}

pub(crate) fn normalize_method_name(value: &str) -> Result<&str> {
    let method = value.trim();
    if method.is_empty() {
        return Err(anyhow!("method name cannot be blank"));
    }
    if method.contains('\0') {
        return Err(anyhow!("method name cannot contain NUL bytes"));
    }
    if !is_valid_method_identifier(method) {
        return Err(anyhow!(
            "method name must be a valid identifier (letters, digits, underscore)"
        ));
    }
    Ok(method)
}

fn method_template(method_name: &str, kind: RhaiMethodKind) -> String {
    match kind {
        RhaiMethodKind::Reconcile => {
            format!("fn {method_name}(ctx) {{\n    #{{}}\n}}\n")
        }
        RhaiMethodKind::Bashcomp => {
            format!("fn {method_name}(ctx) {{\n    []\n}}\n")
        }
    }
}

fn parse_method_decl_line(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("fn ")?;
    let name_len = rest.chars().take_while(|ch| is_ident_continue(*ch)).count();
    if name_len == 0 {
        return None;
    }
    let name = &rest[..name_len];
    if !is_valid_method_identifier(name) {
        return None;
    }
    let after_name = rest[name_len..].trim_start();
    if !after_name.starts_with('(') {
        return None;
    }
    let close_idx = after_name.find(')')?;
    let args = after_name[1..close_idx].trim();
    if args == "ctx" {
        Some(name)
    } else {
        None
    }
}

fn rename_method_declaration_in_source(source: &str, old_name: &str, new_name: &str) -> Option<String> {
    if !is_valid_method_identifier(old_name) || !is_valid_method_identifier(new_name) {
        return None;
    }
    let discovered = parse_compatible_methods(source);
    if discovered.iter().any(|method| method == new_name) {
        return None;
    }

    let mut match_indices = Vec::new();
    for (idx, segment) in source.split_inclusive('\n').enumerate() {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        if parse_method_decl_line(line) == Some(old_name) {
            match_indices.push(idx);
        }
    }
    if match_indices.len() != 1 {
        return None;
    }

    let target = match_indices[0];
    let mut rebuilt = String::with_capacity(source.len() + new_name.len().saturating_sub(old_name.len()));
    for (idx, segment) in source.split_inclusive('\n').enumerate() {
        if idx != target {
            rebuilt.push_str(segment);
            continue;
        }
        let had_newline = segment.ends_with('\n');
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let renamed = rewrite_decl_line_name(line, old_name, new_name)?;
        rebuilt.push_str(&renamed);
        if had_newline {
            rebuilt.push('\n');
        }
    }
    Some(rebuilt)
}

fn rewrite_decl_line_name(line: &str, old_name: &str, new_name: &str) -> Option<String> {
    let indent_len = line.len().saturating_sub(line.trim_start().len());
    let indent = &line[..indent_len];
    let trimmed = &line[indent_len..];
    let rest = trimmed.strip_prefix("fn ")?;
    let name_len = rest.chars().take_while(|ch| is_ident_continue(*ch)).count();
    if name_len == 0 {
        return None;
    }
    let current = &rest[..name_len];
    if current != old_name {
        return None;
    }

    let mut rebuilt = String::new();
    rebuilt.push_str(indent);
    rebuilt.push_str("fn ");
    rebuilt.push_str(new_name);
    rebuilt.push_str(&rest[name_len..]);
    Some(rebuilt)
}

fn is_valid_method_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic()) && chars.all(is_ident_continue)
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_method_name, parse_compatible_methods, rename_method_or_create,
        shared_rhai_path_for_alias_doc, sync_method_after_edit, RhaiMethodKind,
    };
    use anyhow::Result;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn derives_shared_rhai_path_from_alias_doc() {
        let temp = TempDir::new().expect("tempdir");
        let path = temp.path().join("foo.toml");
        let rhai = shared_rhai_path_for_alias_doc(&path);
        assert_eq!(rhai, temp.path().join("foo.rhai"));
    }

    #[test]
    fn method_parser_only_keeps_ctx_signature() {
        let methods = parse_compatible_methods(
            "fn reconcile(ctx) {}\nfn complete(other) {}\nfn x(ctx, y) {}\nfn done(ctx) {}\n",
        );
        assert_eq!(methods, vec!["done".to_string(), "reconcile".to_string()]);
    }

    #[test]
    fn sync_method_detects_single_rename() {
        let before = vec!["old_name".to_string()];
        let after = vec!["new_name".to_string()];
        let synced = sync_method_after_edit(&before, &after, Some("old_name"));
        assert_eq!(synced.as_deref(), Some("new_name"));
    }

    #[test]
    fn normalize_method_rejects_invalid_identifiers() {
        assert!(normalize_method_name(" valid_name ").is_ok());
        assert!(normalize_method_name("1bad").is_err());
        assert!(normalize_method_name("").is_err());
    }

    #[test]
    fn rename_or_create_renames_existing_method_when_unambiguous() -> Result<()> {
        let temp = TempDir::new()?;
        let path = temp.path().join("demo.rhai");
        fs::write(&path, "fn old_name(ctx) {\n    #{}\n}\n")?;
        rename_method_or_create(
            &path,
            Some("old_name"),
            "new_name",
            RhaiMethodKind::Reconcile,
        )?;
        let updated = fs::read_to_string(&path)?;
        assert!(updated.contains("fn new_name(ctx)"));
        assert!(!updated.contains("fn old_name(ctx)"));
        Ok(())
    }
}
