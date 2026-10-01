//! Data for the CodeZ "Code map" panel: the file/import graph from
//! `.agentz/graph.json` enriched with line counts and recent git churn, plus a
//! module-level (directory group) aggregation that stays small on huge repos.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use crate::commands::data_scope::require_project_dir;
use crate::commands::graph::{load_graph, GraphDoc};

const CHURN_DAYS: u32 = 90;
const MAX_LOC_BYTES: u64 = 1_000_000;
const MAX_FILE_EDGES: usize = 8000;

#[derive(Debug, Clone, Serialize)]
pub struct CodeMapFile {
    pub id: String,
    pub path: String,
    pub name: String,
    pub group: String,
    pub layer: String,
    pub loc: usize,
    pub churn: u32,
    pub imports: usize,
    pub imported_by: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodeMapEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodeMapGroup {
    pub name: String,
    pub files: usize,
    pub loc: usize,
    pub churn: u32,
    pub imported_by: usize,
    pub layer: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodeMapGroupEdge {
    pub from: String,
    pub to: String,
    pub weight: usize,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CodeMapData {
    /// False while `graph.json` does not exist yet (a build was queued).
    pub ready: bool,
    pub generated_at: Option<String>,
    pub churn_days: u32,
    pub churn_available: bool,
    pub files: Vec<CodeMapFile>,
    pub edges: Vec<CodeMapEdge>,
    pub edges_truncated: bool,
    pub groups: Vec<CodeMapGroup>,
    pub group_edges: Vec<CodeMapGroupEdge>,
}

/// `src-tauri/src/commands/chat.rs` -> `src-tauri/src`; root files -> `(root)`.
fn group_of(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.len() {
        0 | 1 => "(root)".to_string(),
        2 => parts[0].to_string(),
        _ => format!("{}/{}", parts[0], parts[1]),
    }
}

fn count_lines(full: &Path) -> usize {
    let Ok(meta) = std::fs::metadata(full) else {
        return 0;
    };
    if meta.len() > MAX_LOC_BYTES {
        return 0;
    }
    match std::fs::read(full) {
        Ok(bytes) if !bytes.is_empty() => {
            bytes.iter().filter(|b| **b == b'\n').count() + usize::from(!bytes.ends_with(b"\n"))
        }
        _ => 0,
    }
}

fn dominant<'a>(counts: impl Iterator<Item = &'a str>) -> String {
    let mut map: HashMap<&str, usize> = HashMap::new();
    for l in counts {
        *map.entry(l).or_default() += 1;
    }
    map.into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(a.0)))
        .map(|(l, _)| l.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn build_code_map(
    doc: &GraphDoc,
    root: &Path,
    churn: Option<&HashMap<String, u32>>,
) -> CodeMapData {
    let mut files: Vec<CodeMapFile> = doc
        .nodes
        .iter()
        .filter(|n| n.kind == "file")
        .filter_map(|n| {
            let path = n.path.clone()?;
            Some(CodeMapFile {
                id: n.id.clone(),
                group: group_of(&path),
                layer: n.layer.clone(),
                loc: count_lines(&root.join(&path)),
                churn: churn.and_then(|c| c.get(&path)).copied().unwrap_or(0),
                name: n.name.clone(),
                path,
                imports: 0,
                imported_by: 0,
            })
        })
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));

    let index: HashMap<String, usize> = files
        .iter()
        .enumerate()
        .map(|(i, f)| (f.id.clone(), i))
        .collect();

    let mut edges = Vec::new();
    let mut group_weights: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut truncated = false;
    for e in doc.edges.iter().filter(|e| e.kind == "imports") {
        let (Some(&a), Some(&b)) = (index.get(&e.from), index.get(&e.to)) else {
            continue;
        };
        files[a].imports += 1;
        files[b].imported_by += 1;
        let (ga, gb) = (files[a].group.clone(), files[b].group.clone());
        if ga != gb {
            *group_weights.entry((ga, gb)).or_default() += 1;
        }
        if edges.len() < MAX_FILE_EDGES {
            edges.push(CodeMapEdge {
                from: e.from.clone(),
                to: e.to.clone(),
            });
        } else {
            truncated = true;
        }
    }

    let mut by_group: BTreeMap<String, Vec<&CodeMapFile>> = BTreeMap::new();
    for f in &files {
        by_group.entry(f.group.clone()).or_default().push(f);
    }
    let groups = by_group
        .into_iter()
        .map(|(name, fs)| CodeMapGroup {
            files: fs.len(),
            loc: fs.iter().map(|f| f.loc).sum(),
            churn: fs.iter().map(|f| f.churn).sum(),
            imported_by: fs.iter().map(|f| f.imported_by).sum(),
            layer: dominant(fs.iter().map(|f| f.layer.as_str())),
            name,
        })
        .collect();

    CodeMapData {
        ready: true,
        generated_at: Some(doc.generated_at.clone()),
        churn_days: CHURN_DAYS,
        churn_available: churn.is_some(),
        files,
        edges,
        edges_truncated: truncated,
        groups,
        group_edges: group_weights
            .into_iter()
            .map(|((from, to), weight)| CodeMapGroupEdge { from, to, weight })
            .collect(),
    }
}

/// Commits touching each path in the last [`CHURN_DAYS`] days.
async fn git_churn(root: &Path) -> Option<HashMap<String, u32>> {
    let mut cmd = piscis_kernel::proc::tokio_command("git");
    cmd.current_dir(root)
        .args([
            "log",
            &format!("--since={CHURN_DAYS}.days.ago"),
            "--name-only",
            "--no-renames",
            "--pretty=format:",
        ])
        .kill_on_drop(true);
    let out = tokio::time::timeout(Duration::from_secs(20), cmd.output())
        .await
        .ok()?
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let mut counts: HashMap<String, u32> = HashMap::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let l = line.trim();
        if !l.is_empty() {
            *counts.entry(l.replace('\\', "/")).or_default() += 1;
        }
    }
    Some(counts)
}

#[tauri::command]
pub async fn code_map_data(project_dir: Option<String>) -> Result<CodeMapData, String> {
    let project = require_project_dir(project_dir.as_deref())?;
    let root = PathBuf::from(project);

    let load_root = root.clone();
    let doc = tokio::task::spawn_blocking(move || load_graph(&load_root))
        .await
        .map_err(|e| format!("code map task failed: {e}"))?;
    let Some(doc) = doc else {
        let ensure_root = root.clone();
        let _ = tokio::task::spawn_blocking(move || {
            crate::commands::graph_index::ensure_started(&ensure_root)
        })
        .await;
        return Ok(CodeMapData {
            churn_days: CHURN_DAYS,
            ..Default::default()
        });
    };

    let churn = git_churn(&root).await;
    tokio::task::spawn_blocking(move || build_code_map(&doc, &root, churn.as_ref()))
        .await
        .map_err(|e| format!("code map task failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::graph::{GraphEdge, GraphNode, GraphStats};

    fn file(path: &str, layer: &str) -> GraphNode {
        GraphNode {
            id: format!("file:{path}"),
            kind: "file".into(),
            path: Some(path.into()),
            name: path.rsplit('/').next().unwrap().into(),
            layer: layer.into(),
            summary: String::new(),
        }
    }

    #[test]
    fn aggregates_groups_degrees_and_churn() {
        let doc = GraphDoc {
            version: "1.0".into(),
            generated_at: "t".into(),
            project: "p".into(),
            nodes: vec![
                file("src/ui/a.tsx", "ui"),
                file("src/ui/b.tsx", "ui"),
                file("src-tauri/src/x.rs", "service"),
            ],
            edges: vec![
                GraphEdge { from: "file:src/ui/a.tsx".into(), to: "file:src/ui/b.tsx".into(), kind: "imports".into() },
                GraphEdge { from: "file:src/ui/a.tsx".into(), to: "file:src-tauri/src/x.rs".into(), kind: "imports".into() },
                GraphEdge { from: "module:src".into(), to: "file:src/ui/a.tsx".into(), kind: "contains".into() },
            ],
            modules: vec![],
            stats: GraphStats { files: 3, nodes: 3, edges: 3 },
        };
        let churn: HashMap<String, u32> = [("src/ui/a.tsx".to_string(), 7)].into();
        let map = build_code_map(&doc, Path::new("/nonexistent"), Some(&churn));

        assert_eq!(map.files.len(), 3);
        assert_eq!(map.edges.len(), 2, "only file->file import edges");
        let a = map.files.iter().find(|f| f.path == "src/ui/a.tsx").unwrap();
        assert_eq!((a.imports, a.imported_by, a.churn), (2, 0, 7));
        let ui = map.groups.iter().find(|g| g.name == "src/ui").unwrap();
        assert_eq!((ui.files, ui.churn, ui.imported_by), (2, 7, 1));
        assert_eq!(map.group_edges.len(), 1);
        assert_eq!(map.group_edges[0].from, "src/ui");
        assert_eq!(map.group_edges[0].to, "src-tauri/src");
        assert!(map.churn_available);
    }

    #[test]
    fn group_depth_is_two_components() {
        assert_eq!(group_of("main.rs"), "(root)");
        assert_eq!(group_of("src/App.tsx"), "src");
        assert_eq!(group_of("src-tauri/src/commands/chat.rs"), "src-tauri/src");
    }
}
