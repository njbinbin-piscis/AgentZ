---
name: "deep-research"
description: "Research workflow for analyzing a codebase, doing deep web research and generating a technical wiki. Use when you need an in-depth research report on a project or technology."
description_zh: "研究型工作流：分析代码库、联网深度调研并生成技术 Wiki，适用于需要深入研究报告的项目或技术。"
version: "1.1.0"
source: agentz
---

# Deep Research Workflow

A research workflow that takes a project from codebase analysis, through deep external research, to a complete technical wiki. Every phase uses builtin tools only: `file_search`, `file_read`, `file_write`, `web_search`, `web_fetch`, `shell`, `delegate`.

## Phases

1. **Codebase research analysis**: analyze the codebase and mark places that need web research.
2. **Deep research loop**: resolve every research marker with real sources.
3. **Wiki generation**: produce a complete technical analysis document.
4. **GitHub repository research** (optional): clone and analyze a referenced repository.

Run the phases in order. Tell the user what each phase produced before starting the next one.

## Phase 1: Codebase research analysis

Analyze the codebase and write `deep-search.md` files (project root plus important subdirectories). Wherever web research is needed, place a marker line `TODO: RESEARCH HERE` with the question to answer.

- Start from `.agentz/REPO_WIKI.md` and `.agentz/rules/` if they exist.
- Limit directory depth (default 2) so the result stays reviewable.
- If `deep-search.md` already exists, update it instead of regenerating it, unless the user asks for a fresh start.

Output: `deep-search.md` files with research markers.

## Phase 2: Deep research loop

Resolve every `TODO: RESEARCH HERE` marker. Repeat up to 3 passes (or the number the user asks for).

- Scan all `deep-search.md` files and list the markers.
- Research each one with `web_search` and `web_fetch`; prefer primary sources (official docs, specs, source repositories).
- Write professional, wiki-style content and replace the marker with it, citing the source URLs.
- Save raw notes under `.research/` (`research-001.md`, `research-002.md`, ...).
- When a GitHub repository link turns up, run Phase 4 for it.

Output: all markers replaced; notes saved under `.research/`.

## Phase 3: Wiki generation

Generate the complete technical wiki.

- Read the PRD if one exists (project root or a path the user gives).
- Read everything under `.research/`.
- Use `.agentz/rules/` and `.agentz/REPO_WIKI.md` as codebase context.
- Write the wiki with sections for overview, architecture, core modules, key flows, and open questions.
- Do 3 verification passes: check completeness, check every claim against its source, fix gaps.
- Finish with optimization suggestions and a short summary.

Output:

- `.agentz/WIKI_DEEP.md`: the full technical analysis document
- `.agentz/wiki-optimization.md`: optimization suggestions
- `.agentz/wiki-summary.md`: summary report

## Phase 4: GitHub repository research

Research a repository by cloning and analyzing it.

- Clone it into `.gh-repo/` with `shell` (`git clone --depth 1`).
- Analyze structure and code; a `delegate` sub-agent is useful for broad read-only exploration.
- Write architecture, core modules, API and features notes.
- Save the results under `.research/gh-repo/<repo-name>/`.

## Resulting files

```
project root/
├── deep-search.md                    # research document (with markers)
├── .research/                        # research results
│   ├── research-001.md
│   └── gh-repo/
│       └── repo-name/
│           ├── architecture.md
│           ├── core-modules.md
│           ├── api-reference.md
│           └── features.md
├── .gh-repo/                         # cloned repositories
└── .agentz/
    ├── REPO_WIKI.md                  # quick module overview
    ├── WIKI_DEEP.md                  # deep wiki
    ├── wiki-optimization.md
    └── wiki-summary.md
```

## Notes

1. **Context first**: make sure the project has useful context, such as `.agentz/REPO_WIKI.md` or rules in `.agentz/rules/`.
2. **Network**: deep research needs access to external resources.
3. **Git access**: Phase 4 needs `git` and access to the repository.
4. **Disk space**: cloned repositories consume space; watch the size of `.gh-repo/`, and add `.gh-repo/` to `.gitignore`.
