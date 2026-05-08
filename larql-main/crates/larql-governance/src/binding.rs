use crate::hash::{hash_text, is_hash_ref};
use serde::{Deserialize, Serialize};
use std::path::Path;
use syn::{Item, Visibility};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BindingError {
    #[error(transparent)]
    Parse(#[from] syn::Error),
    #[error("path is not repo-relative UTF-8")]
    Path,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RustStructBinding {
    pub schema_version: String,
    pub repo_relative_path: String,
    pub module_path: Vec<String>,
    pub item_kind: String,
    pub item_name: String,
    pub visibility: String,
    pub item_source_sha256: String,
    pub canonical_location_hash: String,
    pub rust_struct_binding_hash: String,
}

pub fn derive_struct_bindings(
    repo_relative_path: &Path,
    module_path: &[String],
    source: &str,
) -> Result<Vec<RustStructBinding>, BindingError> {
    let path = repo_relative_path
        .to_str()
        .map(str::to_string)
        .ok_or(BindingError::Path)?;
    let file = syn::parse_file(source)?;
    let mut bindings = Vec::new();

    for item in file.items {
        if let Item::Struct(item_struct) = item {
            let item_name = item_struct.ident.to_string();
            let visibility = visibility_name(&item_struct.vis);
            let item_source =
                extract_struct_source(source, &item_name).unwrap_or(item_name.as_str());
            let item_source_sha256 = hash_text(item_source);
            let location_body = format!(
                "{}::{}::struct::{}::{}",
                path,
                module_path.join("::"),
                item_name,
                visibility
            );
            let canonical_location_hash = hash_text(&location_body);
            let binding_body = format!(
                "{}::{}::{}",
                canonical_location_hash, item_source_sha256, item_name
            );
            let rust_struct_binding_hash = hash_text(&binding_body);
            debug_assert!(is_hash_ref(&rust_struct_binding_hash));
            bindings.push(RustStructBinding {
                schema_version: "larql.governance.rust_struct_binding.v1".to_string(),
                repo_relative_path: path.clone(),
                module_path: module_path.to_vec(),
                item_kind: "struct".to_string(),
                item_name,
                visibility,
                item_source_sha256,
                canonical_location_hash,
                rust_struct_binding_hash,
            });
        }
    }

    Ok(bindings)
}

fn visibility_name(vis: &Visibility) -> String {
    match vis {
        Visibility::Public(_) => "pub".to_string(),
        Visibility::Restricted(_) => "restricted".to_string(),
        Visibility::Inherited => "private".to_string(),
    }
}

fn extract_struct_source<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("struct {name}");
    let name_pos = source.find(&needle)?;
    let start = source[..name_pos]
        .rfind("\n#")
        .map(|pos| pos + 1)
        .unwrap_or_else(|| source[..name_pos].rfind('\n').map_or(0, |pos| pos + 1));
    let after_name = name_pos + needle.len();
    let tail = &source[after_name..];
    let brace_rel = tail.find('{');
    let semi_rel = tail.find(';');
    match (brace_rel, semi_rel) {
        (Some(brace), Some(semi)) if semi < brace => Some(&source[start..after_name + semi + 1]),
        (None, Some(semi)) => Some(&source[start..after_name + semi + 1]),
        (Some(brace), _) => {
            let open = after_name + brace;
            let mut depth = 0usize;
            for (offset, ch) in source[open..].char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            let end = open + offset + ch.len_utf8();
                            return Some(&source[start..end]);
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_struct_binding_from_parsed_source() {
        let source = "pub struct ExecutionToken { pub run_id: RunId }\n";
        let bindings = derive_struct_bindings(
            Path::new("src/generated/types.rs"),
            &["generated".into()],
            source,
        )
        .unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].item_name, "ExecutionToken");
        assert!(bindings[0].rust_struct_binding_hash.starts_with("sha256:"));
    }

    #[test]
    fn path_move_changes_binding_hash() {
        let source = "pub struct ExecutionToken { pub run_id: RunId }\n";
        let a = derive_struct_bindings(Path::new("a.rs"), &[], source).unwrap();
        let b = derive_struct_bindings(Path::new("b.rs"), &[], source).unwrap();
        assert_ne!(a[0].rust_struct_binding_hash, b[0].rust_struct_binding_hash);
    }
}
