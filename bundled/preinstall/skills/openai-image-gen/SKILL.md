---
name: "openai-image-gen"
description: "Batch-generate images through the OpenAI Images API with a random prompt sampler and an HTML thumbnail gallery. Use when you need many image variations."
description_zh: "通过 OpenAI Images API 批量生成图片，带随机提示词采样和 HTML 缩略图画廊"
version: "1.0.0"
source: agentz
---

# OpenAI Image Gen

In the commands below, `<skill-dir>` is this skill's installed directory (`<config>/skills/installed/openai-image-gen`).

Generate a handful of "random but structured" prompts and render them via OpenAI Images API.

## Setup

- Needs env: `OPENAI_API_KEY`

## Run

```bash
python3 <skill-dir>/scripts/gen.py
```

Useful flags:
```bash
python3 <skill-dir>/scripts/gen.py --count 16 --model gpt-image-1.5
python3 <skill-dir>/scripts/gen.py --prompt "ultra-detailed studio photo of a lobster astronaut" --count 4
python3 <skill-dir>/scripts/gen.py --size 1536x1024 --quality high --out-dir ./out/images
```

## Output

- `*.png` images
- `prompts.json` (prompt > file mapping)
- `index.html` (thumbnail gallery)