---
name: "nano-pdf"
description: "Edit PDFs with natural-language instructions using the nano-pdf CLI."
description_zh: "精选热门 AI Agent 技能合集，汇集社区高下载量技能于一处。"
version: "1.0.0"
source: codebuddy
source_plugin: "hot-skills"
---

# nano-pdf

Use `nano-pdf` to apply edits to a specific page in a PDF using a natural-language instruction.

## Quick start

```bash
nano-pdf edit deck.pdf 1 "Change the title to 'Q3 Results' and fix the typo in the subtitle"
```

Notes:
- Page numbers are 0-based or 1-based depending on the tool's version/config; if the result looks off by one, retry with the other.
- Always sanity-check the output PDF before sending it out.