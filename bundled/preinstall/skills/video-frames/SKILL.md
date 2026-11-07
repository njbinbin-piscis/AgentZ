---
name: "video-frames"
description: "Extract single frames or thumbnails from a video with ffmpeg. Use when you need to inspect what happens at a given moment of a video."
description_zh: "用 ffmpeg 从视频中提取指定时刻的画面或缩略图"
version: "1.0.0"
source: agentz
---

# Video Frames (ffmpeg)

Extract a single frame from a video, or create quick thumbnails for inspection.

## Quick start

First frame:
```bash
scripts/frame.sh /path/to/video.mp4 --out /tmp/frame.jpg
```

At a timestamp:
```bash
scripts/frame.sh /path/to/video.mp4 --time 00:00:10 --out /tmp/frame-10s.jpg
```

## Notes

- Prefer `--time` for "what is happening around here?".
- Use a `.jpg` for quick share; use `.png` for crisp UI frames.