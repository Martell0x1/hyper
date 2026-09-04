use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::ast::Stmt;
use crate::driver;

/// User-facing path for module search errors (`./foo.hyp` when relative).
fn format_search_path(path: &Path) -> String {
    let s = path.display().to_string();
    if path.is_absolute() || s.starts_with("./") {
        s
    } else {
        format!("./{s}")
    }
}

/// Resolve `math` → `<base>/math.hyp` or `<base>/math/mod.hyp`.
pub fn resolve_module_path(base_dir: &Path, module: &str) -> Result<PathBuf, String> {
    let direct = base_dir.join(format!("{}.hyp", module));
    if direct.is_file() {
        return Ok(direct);
    }
    let nested = base_dir.join(module).join("mod.hyp");
    if nested.is_file() {
        return Ok(nested);
    }
    Err(format!(
        "module '{}' not found (searched {})",
        module,
        format_search_path(&direct)
    ))
}

pub fn read_module_source(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("failed to read {}: {}", path.display(), e))
}

pub fn parse_module_file(path: &Path) -> Result<Vec<Stmt>, String> {
    let source = read_module_source(path)?;
    driver::parse_program(&source).map_err(|_| format!("syntax error in {}", path.display()))
}

pub fn mangle_module_fn(module: &str, func: &str) -> String {
    format!("{}__{}", module, func)
}

/// Members of modules implemented in Rust instead of in a `.hyp` file.
pub fn builtin_module_members(module: &str) -> Option<&'static [&'static str]> {
    match module {
        "json" => Some(&["loads", "dumps", "load", "dump"]),
        _ => None,
    }
}

/// Track in-progress loads to detect import cycles.
pub struct ModuleLoadState {
    pub base_dir: PathBuf,
    pub loading: HashSet<String>,
    /// Canonical path → already-parsed statements (compile path).
    pub parsed: HashMap<String, Vec<Stmt>>,
}

impl ModuleLoadState {
    pub fn new(entry_file: &Path) -> Self {
        let base_dir = entry_file
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        ModuleLoadState {
            base_dir,
            loading: HashSet::new(),
            parsed: HashMap::new(),
        }
    }

    pub fn load_stmts(&mut self, module: &str) -> Result<(PathBuf, Vec<Stmt>), String> {
        let path = resolve_module_path(&self.base_dir, module)?;
        let key = path
            .canonicalize()
            .unwrap_or_else(|_| path.clone())
            .to_string_lossy()
            .into_owned();

        if self.loading.contains(&key) {
            return Err(format!("circular import involving module '{}'", module));
        }
        if let Some(stmts) = self.parsed.get(&key) {
            return Ok((path, stmts.clone()));
        }

        self.loading.insert(key.clone());
        let stmts = parse_module_file(&path)?;
        self.loading.remove(&key);
        self.parsed.insert(key, stmts.clone());
        Ok((path, stmts))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn missing_module_names_searched_file() {
        let err = resolve_module_path(Path::new("."), "foo").unwrap_err();
        assert_eq!(err, "module 'foo' not found (searched ./foo.hyp)");
    }

    #[test]
    fn missing_module_from_file_parent_uses_dot_slash() {
        let err = resolve_module_path(Path::new("test.hyp").parent().unwrap(), "foo").unwrap_err();
        assert_eq!(err, "module 'foo' not found (searched ./foo.hyp)");
    }
}
