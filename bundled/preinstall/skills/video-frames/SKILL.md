---
name: "video-frames"
description: "Extract frames or short clips from videos using ffmpeg."
description_zh: "精选热门 AI Agent 技能合集，汇集社区高下载量技能于一处。"
version: "1.0.0"
source: codebuddy
source_plugin: "hot-skills"
---

# Video Frames (ffmpeg)

Extract a single frame from a video, or create quick thumbnails for inspection.

## Quick start

First frame:
```bash
{baseDir}/scripts/frame.sh /path/to/video.mp4 --out /tmp/frame.jpg
```

At a timestamp:
```bash
{baseDir}/scripts/frame.sh /path/to/video.mp4 --time 00:00:10 --out /tmp/frame-10s.jpg
```

## Notes

- Prefer `--time` for "what is happening around here?".
- Use a `.jpg` for quick share; use `.png` for crisp UI frames.