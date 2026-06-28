//! Repo knowledge graph (P0).
//!
//! Builds `{project}/.agentz/graph.json` from static import analysis.
//! Structural edges are deterministic; `summary` / rich `layer` labels are
//! filled later by LLM enrichment (P1+).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::commands::data_scope::require_project_dir;

const GRAPH_VERSION: &str = "1.0";
const MAX_FILE_BYTES: u64 = 1_000_000;

const CODE_EXTS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "mjs", "cjs", "py", "go", "java", "c", "h", "cpp", "cc",
    "cxx", "hpp", "cs", "rb", "php", "swift", "kt", "scala", "lua", "sh", "bash", "vue", "sql",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphDoc {
    pub version: String,
    pub generated_at: String,
    pub project: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub modules: Vec<ModuleStat>,
    pub stats: GraphStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub name: String,
    pub layer: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleStat {
    pub name: String,
    pub file_count: usize,
    pub in_degree: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub files: usize,
    pub nodes: usize,
    pub edges: usize,
}

fn graph_path(root: &Path) -> PathBuf {
    root.join(".agentz").join("graph.json")
}

fn tours_path(root: &Path) -> PathBuf {
    root.join(".agentz").join("tours.json")
}

fn is_ignored_dir(name: &str) -> bool {
    crate::path_filter::is_ignored_dir_name(name)
}

fn is_code_file(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => CODE_EXTS.contains(&ext.to_lowercase().as_str()),
        None => false,
    }
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn top_module(path: &str) -> String {
    match path.split_once('/') {
        Some((head, _)) => head.to_string(),
        None => "(root)".to_string(),
    }
}

fn file_node_id(rel: &str) -> String {
    format!("file:{rel}")
}

fn module_node_id(name: &str) -> String {
    format!("module:{name}")
}

fn infer_layer(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.contains("/api/")
        || lower.contains("/routes/")
        || lower.contains("/gateway/")
        || lower.contains("controller")
        || lower.ends_with("_api.rs")
        || lower.ends_with("_api.ts")
    {
        "api"
    } else if lower.contains("/service/")
        || lower.contains("/services/")
        || lower.contains("/handler")
        || lower.contains("/commands/")
        || lower.contains("/runtime/")
        || lower.contains("/tools/")
    {
        "service"
    } else if lower.contains("/db/")
        || lower.contains("/data/")
        || lower.contains("/model/")
        || lower.contains("/models/")
        || lower.contains("/schema/")
        || lower.contains("/storage/")
    {
        "data"
    } else if lower.ends_with(".tsx")
        || lower.ends_with(".vue")
        || lower.contains("/ui/")
        || lower.contains("/components/")
        || lower.contains("/frontend/")
        || lower.contains("/workspaces/")
    {
        "ui"
    } else if lower.contains("/util/")
        || lower.contains("/utils/")
        || lower.contains("/helper/")
        || lower.contains("/common/")
        || lower.contains("/hooks/")
        || lower.contains("/store/")
        || lower.contains("/config/")
        || lower.contains("/i18n/")
    {
        "utility"
    } else {
        "unknown"
    }
}

fn collect_files(root: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 12 || out.len() > 20_000 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            if is_ignored_dir(&name) {
                continue;
            }
            collect_files(&path, out, depth + 1);
        } else if meta.is_file() && meta.len() <= MAX_FILE_BYTES && is_code_file(&path) {
            out.push(path);
        }
    }
}

fn normalize_rel_path(rel: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let normalized = rel.replace('\\', "/");
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            _ => out.push(part),
        }
    }
    out.join("/")
}

fn parent_rel_path(from_rel: &str) -> String {
    Path::new(from_rel)
        .parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

fn join_relative(from_rel: &str, spec: &str) -> String {
    if spec.starts_with('/') {
        return normalize_rel_path(spec.trim_start_matches('/'));
    }
    let parent = parent_rel_path(from_rel);
    let raw = if parent.is_empty() {
        spec.to_string()
    } else {
        format!("{parent}/{spec}")
    };
    normalize_rel_path(&raw)
}

fn extract_ts_js_import_spec(trimmed: &str) -> Option<String> {
    if trimmed.starts_with("import ") && !trimmed.contains(" from ") {
        for (open, close) in [('"', '"'), ('\'', '\'')] {
            let start = trimmed.find(open)? + 1;
            let rest = &trimmed[start..];
            let end = rest.find(close)?;
            let spec = rest[..end].trim();
            if spec.starts_with('.') || spec.starts_with('/') {
                return Some(spec.to_string());
            }
        }
        return None;
    }
    let tail = trimmed.rsplit(" from ").next()?;
    let tail = tail.trim().trim_end_matches(';');
    for (open, close) in [('"', '"'), ('\'', '\'')] {
        if let Some(stripped) = tail.strip_prefix(open) {
            if let Some(end) = stripped.find(close) {
                return Some(stripped[..end].to_string());
            }
        }
    }
    None
}

fn rust_path_candidates(from_rel: &str, pathish: &str) -> Vec<String> {
    let parts: Vec<&str> = from_rel.split('/').collect();
    let mut out = Vec::new();
    for end in (1..parts.len()).rev() {
        let dir = parts[..end].join("/");
        for suffix in [".rs", "/mod.rs"] {
            out.push(normalize_rel_path(&format!("{dir}/{pathish}{suffix}")));
        }
    }
    out
}

fn lookup_rel(file_index: &HashMap<String, ()>, rel: &str) -> Option<String> {
    if file_index.contains_key(rel) {
        return Some(rel.to_string());
    }
    None
}

fn candidate_rel_paths(base: &str) -> Vec<String> {
    [
        base.to_string(),
        format!("{base}.rs"),
        format!("{base}.ts"),
        format!("{base}.tsx"),
        format!("{base}.js"),
        format!("{base}.jsx"),
        format!("{base}/mod.rs"),
        format!("{base}/index.ts"),
        format!("{base}/index.tsx"),
        format!("{base}/index.js"),
        format!("{base}.py"),
    ]
    .into_iter()
    .map(|p| normalize_rel_path(&p))
    .collect()
}

/// Extract import target strings from source (best-effort parsing).
fn extract_imports(content: &str, rel: &str) -> Vec<String> {
    let ext = rel.rsplit('.').next().unwrap_or("");
    let mut out = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }
        match ext {
            "rs" => {
                if let Some(rest) = trimmed.strip_prefix("use ") {
                    let spec = rest.trim_end_matches(';').split('{').next().unwrap_or("").trim();
                    if spec.starts_with("crate::") {
                        out.push(spec.replace("crate::", "").to_string());
                    } else if spec.starts_with("super::")
                        || spec.starts_with("self::")
                        || (!spec.is_empty() && spec != "super")
                    {
                        out.push(spec.to_string());
                    }
                } else if trimmed.starts_with("mod ") {
                    let name = trimmed
                        .strip_prefix("mod ")
                        .unwrap_or("")
                        .trim_end_matches(';')
                        .split('{')
                        .next()
                        .unwrap_or("")
                        .trim();
                    if !name.is_empty() {
                        out.push(format!("mod:{name}"));
                    }
                }
            }
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "vue" => {
                if let Some(spec) = extract_ts_js_import_spec(trimmed) {
                    out.push(spec);
                } else if trimmed.contains("require(") {
                    if let Some(q) = trimmed.split('"').nth(1) {
                        out.push(q.to_string());
                    } else if let Some(q) = trimmed.split('\'').nth(1) {
                        out.push(q.to_string());
                    }
                }
            }
            "py" => {
                if let Some(rest) = trimmed.strip_prefix("from ") {
                    let pkg = rest.split_whitespace().next().unwrap_or("").trim();
                    if !pkg.is_empty() {
                        out.push(pkg.to_string());
                    }
                } else if let Some(rest) = trimmed.strip_prefix("import ") {
                    let pkg = rest.split_whitespace().next().unwrap_or("").trim_end_matches(';');
                    if !pkg.is_empty() {
                        out.push(pkg.to_string());
                    }
                }
            }
            "go" if trimmed.starts_with("import ") => {
                let block = trimmed.trim_start_matches("import ").trim();
                for q in block.split('"').skip(1).step_by(2) {
                    out.push(q.to_string());
                }
            }
            _ => {}
        }
    }
    out
}

fn resolve_import(_root: &Path, from_rel: &str, spec: &str, file_index: &HashMap<String, ()>) -> Option<String> {
    if spec.starts_with("mod:") {
        let name = spec.trim_start_matches("mod:");
        let base = parent_rel_path(from_rel);
        for candidate in [
            normalize_rel_path(&format!("{base}/{name}.rs")),
            normalize_rel_path(&format!("{base}/{name}/mod.rs")),
        ] {
            if let Some(hit) = lookup_rel(file_index, &candidate) {
                return Some(hit);
            }
        }
        return None;
    }

    // External npm / std — skip (relative and crate-internal only)
    if !spec.starts_with('.') && !spec.starts_with('/') && !spec.contains("::") {
        return None;
    }

    if spec.contains("::") {
        let pathish = spec.replace("::", "/");
        for candidate in rust_path_candidates(from_rel, &pathish) {
            if let Some(hit) = lookup_rel(file_index, &candidate) {
                return Some(hit);
            }
        }
        return None;
    }

    let joined_rel = join_relative(from_rel, spec);
    for candidate in candidate_rel_paths(&joined_rel) {
        if let Some(hit) = lookup_rel(file_index, &candidate) {
            return Some(hit);
        }
    }
    None
}

struct BuildState {
    file_rels: Vec<String>,
    file_set: BTreeSet<String>,
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
    file_imports: BTreeMap<String, Vec<String>>,
}

impl BuildState {
    fn new(files: &[PathBuf], root: &Path) -> Self {
        let mut file_rels: Vec<String> = files
            .iter()
            .map(|p| rel_path(root, p))
            .filter(|r| !crate::path_filter::is_ignored_rel_path(r))
            .collect();
        file_rels.sort();
        file_rels.dedup();
        let file_set: BTreeSet<String> = file_rels.iter().cloned().collect();
        Self {
            file_rels,
            file_set,
            nodes: Vec::new(),
            edges: Vec::new(),
            file_imports: BTreeMap::new(),
        }
    }

    fn file_index(&self) -> HashMap<String, ()> {
        self.file_set.iter().map(|r| (r.clone(), ())).collect()
    }

    fn ingest_file(&mut self, root: &Path, rel: &str) {
        if !self.file_set.contains(rel) {
            return;
        }
        let full = root.join(rel);
        let Ok(raw) = std::fs::read(&full) else {
            return;
        };
        if raw.len() as u64 > MAX_FILE_BYTES || raw[..raw.len().min(8192)].contains(&0) {
            return;
        }
        let content = String::from_utf8_lossy(&raw);
        let imports_raw = extract_imports(&content, rel);
        let file_index = self.file_index();
        let mut resolved = Vec::new();
        for spec in imports_raw {
            if let Some(target) = resolve_import(root, rel, &spec, &file_index) {
                if target != rel {
                    resolved.push(target);
                }
            }
        }
        resolved.sort();
        resolved.dedup();
        self.file_imports.insert(rel.to_string(), resolved);
    }

    fn build_all_files(&mut self, root: &Path) {
        for rel in self.file_rels.clone() {
            self.ingest_file(root, &rel);
        }
    }

    fn assemble_nodes_edges(&mut self) {
        self.nodes.clear();
        self.edges.clear();

        let mut modules: BTreeSet<String> = BTreeSet::new();
        for rel in &self.file_rels {
            let module = top_module(rel);
            modules.insert(module.clone());

            let name = Path::new(rel)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| rel.clone());
            self.nodes.push(GraphNode {
                id: file_node_id(rel),
                kind: "file".into(),
                path: Some(rel.clone()),
                name,
                layer: infer_layer(rel).into(),
                summary: String::new(),
            });

            self.edges.push(GraphEdge {
                from: module_node_id(&module),
                to: file_node_id(rel),
                kind: "contains".into(),
            });
        }

        for module in &modules {
            self.nodes.push(GraphNode {
                id: module_node_id(module),
                kind: "module".into(),
                path: None,
                name: module.clone(),
                layer: "unknown".into(),
                summary: String::new(),
            });
        }

        for (from_rel, targets) in &self.file_imports {
            for to_rel in targets {
                self.edges.push(GraphEdge {
                    from: file_node_id(from_rel),
                    to: file_node_id(to_rel),
                    kind: "imports".into(),
                });
                let from_mod = top_module(from_rel);
                let to_mod = top_module(to_rel);
                if from_mod != to_mod {
                    self.edges.push(GraphEdge {
                        from: module_node_id(&from_mod),
                        to: module_node_id(&to_mod),
                        kind: "depends".into(),
                    });
                }
            }
        }

        // Dedupe edges
        let mut seen = BTreeSet::new();
        self.edges.retain(|e| seen.insert((e.from.clone(), e.to.clone(), e.kind.clone())));
    }

    fn module_stats(&self) -> Vec<ModuleStat> {
        let mut file_counts: BTreeMap<String, usize> = BTreeMap::new();
        for rel in &self.file_rels {
            *file_counts.entry(top_module(rel)).or_default() += 1;
        }
        let mut in_degree: BTreeMap<String, usize> = BTreeMap::new();
        for e in &self.edges {
            if e.kind != "depends" {
                continue;
            }
            if let Some(to_name) = e.to.strip_prefix("module:") {
                *in_degree.entry(to_name.to_string()).or_default() += 1;
            }
        }
        let mut stats: Vec<ModuleStat> = file_counts
            .into_iter()
            .map(|(name, file_count)| ModuleStat {
                in_degree: in_degree.get(&name).copied().unwrap_or(0),
                name,
                file_count,
            })
            .collect();
        stats.sort_by_key(|b| std::cmp::Reverse(b.file_count));
        stats
    }

    fn into_doc(self, root: &Path) -> GraphDoc {
        let project = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "workspace".into());
        let modules = self.module_stats();
        let files = self.file_rels.len();
        let nodes = self.nodes.len();
        let edges = self.edges.len();
        GraphDoc {
            version: GRAPH_VERSION.into(),
            generated_at: chrono::Utc::now().to_rfc3339(),
            project,
            nodes: self.nodes,
            edges: self.edges,
            modules,
            stats: GraphStats {
                files,
                nodes,
                edges,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TourStop {
    pub order: usize,
    pub module: String,
    pub node_id: String,
    pub file_count: usize,
    pub in_degree: usize,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TourDoc {
    pub version: String,
    pub generated_at: String,
    pub stops: Vec<TourStop>,
}

/// Build a module reading-order tour from dependency topology.
pub fn generate_tour(root: &Path) -> Result<TourDoc, String> {
    let doc = load_graph(root).ok_or("graph.json not found — rebuild graph first".to_string())?;
    let mut in_degree: BTreeMap<String, usize> = BTreeMap::new();
    let mut adj: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for m in &doc.modules {
        in_degree.entry(m.name.clone()).or_insert(0);
    }
    for e in &doc.edges {
        if e.kind != "depends" {
            continue;
        }
        let Some(from) = e.from.strip_prefix("module:") else {
            continue;
        };
        let Some(to) = e.to.strip_prefix("module:") else {
            continue;
        };
        if from == to {
            continue;
        }
        adj.entry(from.to_string()).or_default().insert(to.to_string());
        *in_degree.entry(to.to_string()).or_default() += 1;
        in_degree.entry(from.to_string()).or_default();
    }
    let mut queue: Vec<String> = in_degree
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(k, _)| k.clone())
        .collect();
    queue.sort();
    let mut order: Vec<String> = Vec::new();
    while let Some(cur) = queue.first().cloned() {
        queue.remove(0);
        order.push(cur.clone());
        if let Some(nexts) = adj.get(&cur) {
            for nxt in nexts {
                if let Some(d) = in_degree.get_mut(nxt) {
                    *d = d.saturating_sub(1);
                    if *d == 0 {
                        queue.push(nxt.clone());
                        queue.sort();
                    }
                }
            }
        }
    }
    for (name, deg) in &in_degree {
        if *deg > 0 && !order.contains(name) {
            order.push(name.clone());
        }
    }
    let module_map: BTreeMap<String, &ModuleStat> =
        doc.modules.iter().map(|m| (m.name.clone(), m)).collect();
    let stops: Vec<TourStop> = order
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            let stat = module_map.get(&name);
            TourStop {
                order: i + 1,
                module: name.clone(),
                node_id: module_node_id(&name),
                file_count: stat.map(|s| s.file_count).unwrap_or(0),
                in_degree: stat.map(|s| s.in_degree).unwrap_or(0),
                summary: String::new(),
            }
        })
        .collect();
    let tour = TourDoc {
        version: "1.0".into(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        stops,
    };
    let dir = root.join(".agentz");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create .agentz: {e}"))?;
    let json = serde_json::to_string_pretty(&tour).map_err(|e| format!("serialize tour: {e}"))?;
    std::fs::write(tours_path(root), json).map_err(|e| format!("write tours: {e}"))?;
    Ok(tour)
}

/// Load an existing graph if present.
pub fn load_graph(root: &Path) -> Option<GraphDoc> {
    let path = graph_path(root);
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn load_tour(root: &Path) -> Option<TourDoc> {
    let raw = std::fs::read_to_string(tours_path(root)).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_graph(root: &Path, doc: &GraphDoc) -> Result<(), String> {
    let dir = root.join(".agentz");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create .agentz: {e}"))?;
    let json = serde_json::to_string_pretty(doc).map_err(|e| format!("serialize graph: {e}"))?;
    std::fs::write(graph_path(root), json).map_err(|e| format!("write graph: {e}"))?;
    let _ = crate::commands::graph_db::sync_from_graph_doc(root, doc);
    Ok(())
}

/// Full graph rebuild for `root`.
pub fn generate(root: &Path) -> Result<GraphDoc, String> {
    let mut files = Vec::new();
    collect_files(root, &mut files, 0);
    if files.is_empty() {
        return Err("no source files found — nothing to graph".into());
    }
    let mut state = BuildState::new(&files, root);
    state.build_all_files(root);
    state.assemble_nodes_edges();
    let mut doc = state.into_doc(root);
    let _ = crate::commands::graph_agent::merge_deep_wiki_summaries(&mut doc, root);
    write_graph(root, &doc)?;
    crate::commands::graph_agent::write_agent_artifacts(root, &doc)?;
    Ok(doc)
}

/// File watcher entry — debounced background rebuild via [`graph_index`].
pub fn schedule_patch(root: PathBuf, rel: String) {
    let rel = rel.replace('\\', "/");
    if crate::path_filter::is_ignored_rel_path(&rel) || !is_code_file(Path::new(&rel)) {
        return;
    }
    crate::commands::graph_index::notify_file_change(root, rel);
}

// ─── Tauri commands ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn graph_index_rebuild(
    project_dir: Option<String>,
) -> Result<crate::commands::graph_index::IndexBuildAck, String> {
    let project = require_project_dir(project_dir.as_deref())?;
    Ok(crate::commands::graph_index::request_rebuild(PathBuf::from(project)))
}

#[tauri::command]
pub async fn graph_index_status(
    project_dir: Option<String>,
) -> Result<crate::commands::graph_index::IndexBuildStatus, String> {
    let project = require_project_dir(project_dir.as_deref())?;
    Ok(crate::commands::graph_index::status(Path::new(&project)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_layer_api_path() {
        assert_eq!(infer_layer("src/api/handlers.rs"), "api");
        assert_eq!(infer_layer("src/components/Button.tsx"), "ui");
    }

    #[test]
    fn extract_rust_use() {
        let src = "use crate::commands::codebase;\nuse super::foo;\n";
        let imports = extract_imports(src, "src/lib.rs");
        assert!(imports.iter().any(|i| i.contains("commands")));
    }

    #[test]
    fn resolve_parent_relative_import() {
        let mut index = HashMap::new();
        index.insert("src/store/index.ts".to_string(), ());
        index.insert("src/components/Chat/index.tsx".to_string(), ());
        let hit = resolve_import(
            Path::new("/proj"),
            "src/components/Chat/index.tsx",
            "../../store",
            &index,
        );
        assert_eq!(hit.as_deref(), Some("src/store/index.ts"));
    }

    #[test]
    fn openpiscis_src_has_internal_imports() {
        let root = Path::new("/home/nfs/zyh/Projects/dimnuo/all_solutions/openpisci");
        if !root.join("src").is_dir() {
            return;
        }
        let doc = generate(root).expect("graph generate");
        let src_internal = doc
            .edges
            .iter()
            .filter(|e| {
                e.kind == "imports"
                    && e.from.starts_with("file:src/")
                    && e.to.starts_with("file:src/")
            })
            .count();
        assert!(
            src_internal > 100,
            "expected many src internal imports, got {src_internal}"
        );
    }
}
