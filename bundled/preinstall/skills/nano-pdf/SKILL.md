---
name: "nano-pdf"
description: "Edit a page of a PDF from a natural-language instruction with the nano-pdf CLI. Use when you need quick text or layout fixes in an existing PDF."
description_zh: "用自然语言指令通过 nano-pdf 命令行修改 PDF 指定页面，适用于快速修正已有 PDF"
version: "1.0.0"
source: agentz
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