//! Markdown plan documents for CodeZ two-level Plan mode.
//!
//! Level 1 (Plan mode): agent writes a structured plan under `.agentz/plans/`.
//! Level 2 (Agent mode): agent executes steps, updates the same file, and
//! produces evidence/artifacts listed in each step.

use std::path::{Path, PathBuf};

/// Project-relative directory for plan markdown files.
pub const PLANS_DIR: &str = ".agentz/plans";

/// Default plan filename stem when the caller omits an explicit path.
pub fn default_plan_rel_path(session_id: &str) -> String {
    let safe = session_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!("{PLANS_DIR}/{safe}.md")
}

pub fn resolve_plan_path(workspace_root: &Path, rel_or_abs: &str) -> PathBuf {
    let p = Path::new(rel_or_abs);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        workspace_root.join(p)
    }
}

/// Returns true when `path` resolves to `{workspace}/.agentz/plans/*.md`.
pub fn is_allowed_plan_path(workspace_root: &Path, rel_or_abs: &str) -> bool {
    let p = Path::new(rel_or_abs);
    let rel: &Path = if p.is_absolute() {
        if let Ok(r) = p.strip_prefix(workspace_root) {
            r
        } else {
            let root = workspace_root
                .canonicalize()
                .unwrap_or_else(|_| workspace_root.to_path_buf());
            let abs = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
            match abs.strip_prefix(&root) {
                Ok(r) => return rel_plan_under_plans(r),
                Err(_) => return false,
            }
        }
    } else {
        p
    };
    rel_plan_under_plans(rel)
}

fn rel_plan_under_plans(rel: &Path) -> bool {
    if rel
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return false;
    }
    let mut it = rel.components();
    match (it.next(), it.next()) {
        (Some(std::path::Component::Normal(a)), Some(std::path::Component::Normal(b)))
            if a == ".agentz" && b == "plans" => {}
        _ => return false,
    }
    matches!(rel.extension().and_then(|e| e.to_str()), Some("md"))
}

/// Starter template injected when the agent creates a new plan file.
pub fn plan_template(title: &str, session_id: &str) -> String {
    let title = title.trim();
    let title = if title.is_empty() {
        "未命名任务"
    } else {
        title
    };
    format!(
        r#"---
title: "{title}"
session_id: "{session_id}"
status: draft
created: "{date}"
---

# 任务概述

（简述目标、范围、约束与成功标准。）

# 前置调研

（Plan 模式调研结论：关键文件路径、现有实现、风险与 trade-off。）

# 执行步骤

> 每步必须原子化、可独立验收。Agent 模式执行时逐步更新 **状态** 与 **执行记录**，并产出 **预期产物** 对应的 **验收证据**。

## Step 1: example-step — 示例步骤标题

- **状态**: pending
- **描述**: 本步要完成的具体工作（单一职责）。
- **依赖**: 无
- **预期产物**:
  - `path/to/artifact` 或具体交付物描述
- **验收证据**:
  - 测试命令输出 / diff 摘要 / 截图路径 / 日志片段
- **执行记录**:
  - （Agent 模式填写：实际做了什么、证据链接、阻塞原因等）

"#,
        date = chrono::Utc::now().format("%Y-%m-%d"),
    )
}

/// Canonical step skeleton quoted in validation errors so the model can fix
/// the format in one retry instead of guessing.
pub const STEP_FORMAT_HINT: &str = "每个步骤必须是行首恰为两个 # 的标题，形如 `## Step 1: step-id — 标题`（不是 `### Step`、不是 `Step 1.`），\
标题下用加粗字段行：`- **状态**: pending` / `- **描述**: …` / `- **依赖**: …` / `- **预期产物**: …` / `- **验收证据**: …` / `- **执行记录**: …`。";

fn step_heading_level(line: &str) -> Option<usize> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if !(2..=6).contains(&hashes) {
        return None;
    }
    let rest = line[hashes..].trim_start();
    let rest = rest.strip_prefix("Step").or_else(|| rest.strip_prefix("步骤"))?;
    rest.trim_start().chars().next().filter(|c| c.is_ascii_digit())?;
    Some(hashes)
}

/// Normalize near-miss step headings (`### Step 1`, `## 步骤 1`, `##Step 1`) to
/// the canonical `## Step N` form. Fenced code blocks are left untouched.
pub fn normalize_plan_content(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut in_fence = false;
    for line in content.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        }
        if !in_fence && step_heading_level(line).is_some() {
            let hashes = line.chars().take_while(|c| *c == '#').count();
            let rest = line[hashes..].trim_start();
            let rest = rest.strip_prefix("步骤").map(|r| format!("Step{r}")).unwrap_or_else(|| rest.to_string());
            out.push_str("## ");
            out.push_str(&rest);
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// Lightweight structural validation for plan markdown before write.
pub fn validate_plan_content(content: &str) -> Result<(), String> {
    let trimmed = content.trim();
    if trimmed.len() < 80 {
        return Err("计划内容过短，请按模板填写完整计划".into());
    }
    if !trimmed.contains("# 执行步骤") && !trimmed.contains("## Step") {
        return Err(format!("计划必须包含「# 执行步骤」章节。{STEP_FORMAT_HINT}"));
    }
    let step_count = trimmed.lines().filter(|l| l.starts_with("## Step")).count();
    if step_count == 0 {
        return Err(format!("至少定义 1 个原子步骤。{STEP_FORMAT_HINT}"));
    }
    for (i, block) in split_step_blocks(trimmed).iter().enumerate() {
        let n = i + 1;
        if !block.contains("**状态**") {
            return Err(format!("Step {n} 缺少 **状态** 字段"));
        }
        if !block.contains("**预期产物**") {
            return Err(format!("Step {n} 缺少 **预期产物** 字段"));
        }
        if !block.contains("**验收证据**") {
            return Err(format!("Step {n} 缺少 **验收证据** 字段"));
        }
        if !block.contains("**描述**") {
            return Err(format!("Step {n} 缺少 **描述** 字段"));
        }
    }
    Ok(())
}

fn split_step_blocks(content: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current = String::new();
    for line in content.lines() {
        if line.starts_with("## Step") {
            if !current.is_empty() {
                blocks.push(current.clone());
            }
            current.clear();
            current.push_str(line);
            current.push('\n');
        } else if !current.is_empty() {
            current.push_str(line);
            current.push('\n');
        }
    }
    if !current.is_empty() {
        blocks.push(current);
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_path_under_plans_dir() {
        let p = default_plan_rel_path("sess-1");
        assert!(p.starts_with(".agentz/plans/"));
        assert!(p.ends_with(".md"));
    }

    #[test]
    fn validate_rejects_missing_steps() {
        let body = "# 任务概述\n\nx\n\n# 前置调研\n\ny\n\n# 执行步骤\n\n(no steps yet)";
        assert!(validate_plan_content(body).is_err());
    }

    #[test]
    fn normalize_fixes_h3_step_headings_but_not_code_fences() {
        let body = "# 执行步骤\n\n### Step 1: a — 标题\n- **状态**: x\n- **描述**: d\n- **预期产物**: p\n- **验收证据**: e\n\n```\n### Step 9: keep\n```\n\n## 步骤 2: b — 二\n- **状态**: x\n- **描述**: d\n- **预期产物**: p\n- **验收证据**: e\n";
        let n = normalize_plan_content(body);
        assert!(n.contains("\n## Step 1: a"));
        assert!(n.contains("\n### Step 9: keep"));
        assert!(n.contains("\n## Step 2: b"));
        validate_plan_content(&n).expect("normalized plan validates");
    }

    #[test]
    fn validate_accepts_minimal_plan() {
        let content = plan_template("Test task", "sess-1");
        validate_plan_content(&content).expect("template should validate");
    }

    #[test]
    fn is_allowed_plan_path_checks_extension() {
        let root = std::env::temp_dir().join("plan_doc_test_root");
        let _ = std::fs::create_dir_all(root.join(PLANS_DIR));
        assert!(is_allowed_plan_path(&root, ".agentz/plans/foo.md"));
        assert!(!is_allowed_plan_path(&root, ".agentz/plans/foo.txt"));
        assert!(!is_allowed_plan_path(&root, "src/main.rs"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
