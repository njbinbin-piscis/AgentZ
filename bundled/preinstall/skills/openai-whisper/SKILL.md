---
name: "openai-whisper"
description: "Transcribe audio locally with the Whisper CLI, no API key needed. Use when converting speech or recordings to text offline."
description_zh: "用 Whisper 命令行在本地转写音频，无需 API Key，适用于离线语音转文字"
version: "1.0.0"
source: agentz
---

# Whisper (CLI)

Use `whisper` to transcribe audio locally.

Quick start
- `whisper /path/audio.mp3 --model medium --output_format txt --output_dir .`
- `whisper /path/audio.m4a --task translate --output_format srt`

Notes
- Models download to `~/.cache/whisper` on first run.
- `--model` defaults to `turbo` in recent versions.
- Use smaller models for speed, larger for accuracy.