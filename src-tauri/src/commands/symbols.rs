//! Symbol and call-reference index built with tree-sitter (Rust, TS/JS, Python, Go).
//!
//! Backs the `symbol_search` and `impact` agent tools. The index is kept in memory
//! per project and rebuilt when the file set or any mtime changes. Call edges are
//! resolved by name only (no type resolution), so results are a heuristic.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use tree_sitter::{Language, Node, Parser};

const MAX_INDEX_FILES: usize = 6000;
const MAX_PARSE_BYTES: u64 = 600_000;

#[derive(Debug, Clone, PartialEq)]
pub struct Symbol {
    pub name: String,
    pub kind: &'static str,
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    pub container: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CallRef {
    /// Index into the symbol list of the file this call was found in (local index).
    pub caller: Option<usize>,
    pub callee: String,
    pub line: usize,
}

#[derive(Debug, Default)]
pub struct FileSymbols {
    pub symbols: Vec<Symbol>,
    pub calls: Vec<CallRef>,
}

#[derive(Debug, Default)]
pub struct SymbolIndex {
    pub symbols: Vec<Symbol>,
    /// (global caller symbol index, callee name, file, line)
    pub calls: Vec<(Option<usize>, String, String, usize)>,
    pub by_name: HashMap<String, Vec<usize>>,
    pub callers_of: HashMap<String, Vec<usize>>,
}

#[derive(Clone, Copy)]
enum Lang {
    Rust,
    Ts,
    Tsx,
    Js,
    Python,
    Go,
}

fn lang_for(path: &Path) -> Option<Lang> {
    match path.extension()?.to_str()?.to_lowercase().as_str() {
        "rs" => Some(Lang::Rust),
        "ts" | "mts" | "cts" => Some(Lang::Ts),
        "tsx" => Some(Lang::Tsx),
        "js" | "jsx" | "mjs" | "cjs" => Some(Lang::Js),
        "py" => Some(Lang::Python),
        "go" => Some(Lang::Go),
        _ => None,
    }
}

fn language(l: Lang) -> Language {
    match l {
        Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
        Lang::Ts => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Lang::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        Lang::Js => tree_sitter_javascript::LANGUAGE.into(),
        Lang::Python => tree_sitter_python::LANGUAGE.into(),
        Lang::Go => tree_sitter_go::LANGUAGE.into(),
    }
}

fn text<'a>(n: Node<'_>, src: &'a [u8]) -> &'a str {
    n.utf8_text(src).unwrap_or("")
}

fn field_text(n: Node<'_>, field: &str, src: &[u8]) -> Option<String> {
    let t = text(n.child_by_field_name(field)?, src).trim().to_string();
    (!t.is_empty()).then_some(t)
}

fn symbol_kind(l: Lang, n: Node<'_>) -> Option<&'static str> {
    let k = n.kind();
    match l {
        Lang::Rust => match k {
            "function_item" => Some("fn"),
            "struct_item" => Some("struct"),
            "enum_item" => Some("enum"),
            "trait_item" => Some("trait"),
            "mod_item" => Some("mod"),
            "type_item" => Some("type"),
            "const_item" | "static_item" => Some("const"),
            _ => None,
        },
        Lang::Ts | Lang::Tsx | Lang::Js => match k {
            "function_declaration" | "generator_function_declaration" => Some("function"),
            "class_declaration" | "abstract_class_declaration" => Some("class"),
            "method_definition" | "method_signature" => Some("method"),
            "interface_declaration" => Some("interface"),
            "type_alias_declaration" => Some("type"),
            "enum_declaration" => Some("enum"),
            "variable_declarator" => {
                let v = n.child_by_field_name("value")?;
                matches!(v.kind(), "arrow_function" | "function_expression" | "function")
                    .then_some("function")
            }
            _ => None,
        },
        Lang::Python => match k {
            "function_definition" => Some("function"),
            "class_definition" => Some("class"),
            _ => None,
        },
        Lang::Go => match k {
            "function_declaration" => Some("func"),
            "method_declaration" => Some("method"),
            "type_spec" => Some("type"),
            _ => None,
        },
    }
}

fn callee_name(l: Lang, n: Node<'_>, src: &[u8]) -> Option<String> {
    let (call_kind, func_field) = match l {
        Lang::Python => ("call", "function"),
        _ => ("call_expression", "function"),
    };
    if n.kind() != call_kind {
        return None;
    }
    let f = n.child_by_field_name(func_field)?;
    let name = match f.kind() {
        "identifier" | "field_identifier" | "property_identifier" => text(f, src).to_string(),
        "field_expression" => field_text(f, "field", src)?,
        "scoped_identifier" => field_text(f, "name", src)?,
        "member_expression" => field_text(f, "property", src)?,
        "attribute" => field_text(f, "attribute", src)?,
        "selector_expression" => field_text(f, "field", src)?,
        "generic_function" => {
            let inner = f.child_by_field_name("function")?;
            match inner.kind() {
                "identifier" => text(inner, src).to_string(),
                "field_expression" => field_text(inner, "field", src)?,
                "scoped_identifier" => field_text(inner, "name", src)?,
                _ => return None,
            }
        }
        _ => return None,
    };
    (!name.is_empty()).then_some(name)
}

fn walk(
    l: Lang,
    n: Node<'_>,
    src: &[u8],
    file: &str,
    enclosing: Option<usize>,
    container: Option<&str>,
    depth: usize,
    out: &mut FileSymbols,
) {
    if depth > 160 {
        return;
    }
    let mut next_enclosing = enclosing;
    let mut next_container = container.map(str::to_string);

    if let Some(kind) = symbol_kind(l, n) {
        if let Some(name) = field_text(n, "name", src) {
            let idx = out.symbols.len();
            out.symbols.push(Symbol {
                name: name.clone(),
                kind,
                file: file.to_string(),
                start_line: n.start_position().row + 1,
                end_line: n.end_position().row + 1,
                container: container.map(str::to_string),
            });
            if matches!(kind, "fn" | "function" | "method" | "func") {
                next_enclosing = Some(idx);
            }
            if matches!(kind, "struct" | "enum" | "trait" | "class" | "interface" | "mod") {
                next_container = Some(name);
            }
        }
    } else if l_is_rust_impl(l, n) {
        if let Some(t) = field_text(n, "type", src) {
            next_container = Some(t);
        }
    }

    if let Some(callee) = callee_name(l, n, src) {
        out.calls.push(CallRef {
            caller: enclosing,
            callee,
            line: n.start_position().row + 1,
        });
    }

    let mut cursor = n.walk();
    for child in n.children(&mut cursor) {
        walk(
            l,
            child,
            src,
            file,
            next_enclosing,
            next_container.as_deref(),
            depth + 1,
            out,
        );
    }
}

fn l_is_rust_impl(l: Lang, n: Node<'_>) -> bool {
    matches!(l, Lang::Rust) && n.kind() == "impl_item"
}

pub fn extract_file(path: &Path, rel: &str, source: &str) -> Option<FileSymbols> {
    let l = lang_for(path)?;
    let mut parser = Parser::new();
    parser.set_language(&language(l)).ok()?;
    let tree = parser.parse(source, None)?;
    let mut out = FileSymbols::default();
    walk(l, tree.root_node(), source.as_bytes(), rel, None, None, 0, &mut out);
    Some(out)
}

fn collect_source_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > 14 || files.len() >= MAX_INDEX_FILES {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                if crate::path_filter::is_ignored_dir_name(&name) || name == "bundled" {
                    continue;
                }
                stack.push((path, depth + 1));
            } else if meta.is_file() && meta.len() <= MAX_PARSE_BYTES && lang_for(&path).is_some() {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn fingerprint(files: &[PathBuf]) -> u64 {
    let mut h: u64 = files.len() as u64;
    for f in files {
        let m = std::fs::metadata(f)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        h = h.wrapping_mul(1_099_511_628_211).wrapping_add(m ^ (f.as_os_str().len() as u64));
    }
    h
}

pub fn build_index(root: &Path, files: &[PathBuf]) -> SymbolIndex {
    let mut idx = SymbolIndex::default();
    for path in files {
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let Some(fs) = extract_file(path, &rel, &source) else {
            continue;
        };
        let base = idx.symbols.len();
        idx.symbols.extend(fs.symbols);
        for c in fs.calls {
            idx.calls
                .push((c.caller.map(|i| base + i), c.callee, rel.clone(), c.line));
        }
    }
    for (i, s) in idx.symbols.iter().enumerate() {
        idx.by_name.entry(s.name.clone()).or_default().push(i);
    }
    for (i, c) in idx.calls.iter().enumerate() {
        idx.callers_of.entry(c.1.clone()).or_default().push(i);
    }
    idx
}

type Cache = Mutex<HashMap<PathBuf, (u64, Arc<SymbolIndex>)>>;

fn cache() -> &'static Cache {
    static C: OnceLock<Cache> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn index_for(root: &Path) -> Arc<SymbolIndex> {
    let files = collect_source_files(root);
    let fp = fingerprint(&files);
    if let Some((cached_fp, idx)) = cache().lock().ok().and_then(|c| c.get(root).cloned()) {
        if cached_fp == fp {
            return idx;
        }
    }
    let idx = Arc::new(build_index(root, &files));
    if let Ok(mut c) = cache().lock() {
        c.insert(root.to_path_buf(), (fp, idx.clone()));
    }
    idx
}

pub fn search_symbols(idx: &SymbolIndex, query: &str, kind: Option<&str>, limit: usize) -> String {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return "symbol_search: empty query.".into();
    }
    let mut scored: Vec<(i32, usize)> = Vec::new();
    for (i, s) in idx.symbols.iter().enumerate() {
        if let Some(k) = kind {
            if s.kind != k {
                continue;
            }
        }
        let n = s.name.to_lowercase();
        let score = if n == q {
            100
        } else if n.starts_with(&q) {
            70
        } else if n.contains(&q) {
            40
        } else {
            continue;
        };
        scored.push((score - (s.name.len() as i32).min(30) / 3, i));
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    if scored.is_empty() {
        return format!("No symbol matching `{query}` ({} symbols indexed).", idx.symbols.len());
    }
    let mut out = format!("{} match(es) for `{query}`:\n", scored.len().min(limit));
    for (_, i) in scored.into_iter().take(limit) {
        let s = &idx.symbols[i];
        let owner = s
            .container
            .as_ref()
            .map(|c| format!(" (in {c})"))
            .unwrap_or_default();
        out.push_str(&format!(
            "- {} `{}`{} — {}:{}-{}\n",
            s.kind, s.name, owner, s.file, s.start_line, s.end_line
        ));
    }
    out
}

pub fn impact_report(idx: &SymbolIndex, symbol: &str, depth: usize, limit: usize) -> String {
    let Some(defs) = idx.by_name.get(symbol) else {
        return format!(
            "No definition named `{symbol}` found. Try `symbol_search` for similar names."
        );
    };
    let mut out = String::new();
    out.push_str(&format!("Impact of `{symbol}` (name-based call resolution, may over-approximate):\n\nDefined at:\n"));
    for &d in defs.iter().take(8) {
        let s = &idx.symbols[d];
        out.push_str(&format!("- {} {}:{}-{}\n", s.kind, s.file, s.start_line, s.end_line));
    }

    let mut seen: HashSet<String> = HashSet::new();
    seen.insert(symbol.to_string());
    let mut queue: VecDeque<(String, usize)> = VecDeque::from([(symbol.to_string(), 0)]);
    let mut files: HashSet<String> = HashSet::new();
    let mut lines: Vec<String> = Vec::new();
    let mut total = 0usize;

    while let Some((name, d)) = queue.pop_front() {
        if d >= depth {
            continue;
        }
        let Some(call_idxs) = idx.callers_of.get(&name) else {
            continue;
        };
        for &ci in call_idxs {
            let (caller, _, file, line) = &idx.calls[ci];
            total += 1;
            files.insert(file.clone());
            let who = caller
                .map(|c| {
                    let s = &idx.symbols[c];
                    match &s.container {
                        Some(o) => format!("{o}::{}", s.name),
                        None => s.name.clone(),
                    }
                })
                .unwrap_or_else(|| "(top level)".into());
            if lines.len() < limit {
                lines.push(format!("{}- `{who}` at {file}:{line} calls `{name}`", "  ".repeat(d)));
            }
            if let Some(c) = caller {
                let cname = idx.symbols[*c].name.clone();
                if seen.insert(cname.clone()) {
                    queue.push_back((cname, d + 1));
                }
            }
        }
    }

    if total == 0 {
        out.push_str("\nNo callers found (unused, only referenced dynamically, or called from unsupported languages).\n");
    } else {
        out.push_str(&format!(
            "\nCallers (depth {depth}): {total} call site(s) across {} file(s)\n",
            files.len()
        ));
        out.push_str(&lines.join("\n"));
        if total > lines.len() {
            out.push_str(&format!("\n… and {} more", total - lines.len()));
        }
        out.push('\n');
        let mut fl: Vec<_> = files.into_iter().collect();
        fl.sort();
        out.push_str("\nFiles to review/test:\n");
        for f in fl.iter().take(30) {
            out.push_str(&format!("- {f}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idx_of(files: &[(&str, &str)]) -> SymbolIndex {
        let mut idx = SymbolIndex::default();
        for (rel, src) in files {
            let fs = extract_file(Path::new(rel), rel, src).unwrap();
            let base = idx.symbols.len();
            idx.symbols.extend(fs.symbols);
            for c in fs.calls {
                idx.calls
                    .push((c.caller.map(|i| base + i), c.callee, rel.to_string(), c.line));
            }
        }
        for (i, s) in idx.symbols.iter().enumerate() {
            idx.by_name.entry(s.name.clone()).or_default().push(i);
        }
        for (i, c) in idx.calls.iter().enumerate() {
            idx.callers_of.entry(c.1.clone()).or_default().push(i);
        }
        idx
    }

    #[test]
    fn rust_symbols_and_calls() {
        let src = "struct S;\nimpl S {\n    fn run(&self) { helper(); self.other(); }\n}\nfn helper() {}\nfn main() { S.run(); }\n";
        let fs = extract_file(Path::new("a.rs"), "a.rs", src).unwrap();
        let names: Vec<_> = fs.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"S") && names.contains(&"run") && names.contains(&"helper"));
        let run = fs.symbols.iter().find(|s| s.name == "run").unwrap();
        assert_eq!(run.container.as_deref(), Some("S"));
        assert!(fs.calls.iter().any(|c| c.callee == "helper"));
        assert!(fs.calls.iter().any(|c| c.callee == "run"));
    }

    #[test]
    fn typescript_python_go() {
        let ts = extract_file(
            Path::new("a.ts"),
            "a.ts",
            "export function a() { b(); }\nconst b = () => c.d();\nclass K { m() { a(); } }\n",
        )
        .unwrap();
        let names: Vec<_> = ts.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"a") && names.contains(&"b") && names.contains(&"K") && names.contains(&"m"));
        assert!(ts.calls.iter().any(|c| c.callee == "d"));

        let py = extract_file(Path::new("a.py"), "a.py", "def f():\n    g()\nclass C:\n    def m(self):\n        self.f()\n").unwrap();
        assert!(py.symbols.iter().any(|s| s.name == "m"));
        assert!(py.calls.iter().any(|c| c.callee == "g"));

        let go = extract_file(Path::new("a.go"), "a.go", "package p\nfunc F() { G() }\nfunc (r T) M() { r.F2() }\n").unwrap();
        assert!(go.symbols.iter().any(|s| s.name == "M"));
        assert!(go.calls.iter().any(|c| c.callee == "F2"));
    }

    #[test]
    fn impact_walks_callers_transitively() {
        let idx = idx_of(&[
            ("a.rs", "fn leaf() {}\nfn mid() { leaf(); }\n"),
            ("b.rs", "fn top() { mid(); }\n"),
        ]);
        let r = impact_report(&idx, "leaf", 3, 20);
        assert!(r.contains("mid") && r.contains("top") && r.contains("b.rs"), "{r}");
        let s = search_symbols(&idx, "mi", None, 5);
        assert!(s.contains("`mid`"), "{s}");
    }

    #[test]
    fn indexes_this_repository() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let idx = index_for(root);
        assert!(idx.symbols.len() > 200, "symbols: {}", idx.symbols.len());
        let hit = search_symbols(&idx, "build_code_map", None, 5);
        assert!(hit.contains("code_map.rs"), "{hit}");
        let r = impact_report(&idx, "build_code_map", 2, 20);
        assert!(r.contains("code_map_data"), "{r}");
    }
}
