---
name: "openai-whisper-api"
description: "Transcribe audio with the OpenAI Audio Transcriptions API (Whisper) using curl. Use when you have an OpenAI API key and need speech-to-text."
description_zh: "通过 OpenAI 音频转写 API（Whisper）转写音频，适用于已有 OpenAI Key 的语音转文字"
version: "1.0.0"
source: agentz
---

# OpenAI Whisper API (curl)

In the commands below, `<skill-dir>` is this skill's installed directory (`<config>/skills/installed/openai-whisper-api`).

Transcribe an audio file via OpenAI's `/v1/audio/transcriptions` endpoint.

## Quick start

```bash
<skill-dir>/scripts/transcribe.sh /path/to/audio.m4a
```

Defaults:
- Model: `whisper-1`
- Output: `<input>.txt`

## Useful flags

```bash
<skill-dir>/scripts/transcribe.sh /path/to/audio.ogg --model whisper-1 --out /tmp/transcript.txt
<skill-dir>/scripts/transcribe.sh /path/to/audio.m4a --language en
<skill-dir>/scripts/transcribe.sh /path/to/audio.m4a --prompt "Speaker names: Peter, Daniel"
<skill-dir>/scripts/transcribe.sh /path/to/audio.m4a --json --out /tmp/transcript.json
```

## API key

Set `OPENAI_API_KEY` environment variable.