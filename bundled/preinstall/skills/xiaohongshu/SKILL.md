---
name: "xiaohongshu"
description: "Search, read, comment on, track topics and publish to Xiaohongshu (RED) through the local xiaohongshu-mcp service and helper shell scripts. Use when the user works with Xiaohongshu notes."
description_zh: "通过本地 xiaohongshu-mcp 服务和脚本搜索、阅读、评论、发布小红书笔记并做热点跟踪"
version: "1.0.0"
source: agentz
---

# Xiaohongshu MCP Skill

A set of shell scripts wrapping [xiaohongshu-mcp](https://github.com/xpzouying/xiaohongshu-mcp). Run them with the `shell` tool.

## Prerequisites

```bash
cd scripts/
./install-check.sh    # check dependencies (xiaohongshu-mcp, jq, python3)
./start-mcp.sh        # start the MCP service (default port 18060)
./status.sh           # confirm you are logged in
```

If not logged in, scan a QR code: run `mcp-call.sh get_login_qrcode` and scan the code with the Xiaohongshu app.

The service URL can be overridden with the `MCP_URL` environment variable (default `http://localhost:18060/mcp`).

## Core data flow

**Important:** most operations need a `feed_id` + `xsec_token` pair. Both come from search / recommendation / user profile results and **must never be made up**.

```
search_feeds / list_feeds / user_profile
        |
        v
  returns a feeds array; each feed contains:
  +-- id          -> use as feed_id
  +-- xsecToken   -> use as xsec_token
  +-- noteCard    -> title, author, cover, engagement data
        |
        v
  get_feed_detail(feed_id, xsec_token)
        |
        v
  returns the full note: body text, image list, comment list
  comments include comment_id and user_id (needed to reply)
```

## Script reference

| Script | Purpose | Arguments |
|--------|---------|-----------|
| `search.sh <keyword>` | Search notes | keyword |
| `recommend.sh` | Home feed recommendations | none |
| `post-detail.sh <feed_id> <xsec_token>` | Post detail + comments | from search results |
| `comment.sh <feed_id> <xsec_token> <content>` | Post a comment | from search results |
| `user-profile.sh <user_id> <xsec_token>` | User profile + notes | from search results |
| `track-topic.sh <topic> [options]` | Hot topic analysis report | `--limit N` `--output file` `--feishu` |
| `export-long-image.sh` | Export posts as a long image | `--posts-file json -o output.jpg` |
| `mcp-call.sh <tool> [json_args]` | Generic MCP call | see tool details below |
| `start-mcp.sh` | Start the service | `--headless=false` `--port=N` |
| `stop-mcp.sh` | Stop the service | none |
| `status.sh` | Check login | none |
| `install-check.sh` | Check dependencies | none |

## MCP tool details

Filter values below are the literal Chinese strings the service expects; keep them as written.

### search_feeds - search notes

```json
{"keyword": "咖啡", "filters": {"sort_by": "最新", "note_type": "图文", "publish_time": "一周内"}}
```

Optional filters:
- `sort_by`: 综合 (overall) | 最新 (newest) | 最多点赞 (most likes) | 最多评论 (most comments) | 最多收藏 (most saves)
- `note_type`: 不限 (any) | 视频 (video) | 图文 (image-text)
- `publish_time`: 不限 (any) | 一天内 (1 day) | 一周内 (1 week) | 半年内 (6 months)
- `search_scope`: 不限 (any) | 已看过 (seen) | 未看过 (unseen) | 已关注 (following)
- `location`: 不限 (any) | 同城 (same city) | 附近 (nearby)

### get_feed_detail - post detail

```json
{"feed_id": "...", "xsec_token": "...", "load_all_comments": true, "limit": 20}
```

- `load_all_comments`: false (default) returns the first 10 comments; true scrolls to load more
- `limit`: maximum comments to load (only with load_all_comments=true), default 20
- `click_more_replies`: expand second-level replies, default false
- `reply_limit`: skip comments whose reply count exceeds this value, default 10
- `scroll_speed`: slow | normal | fast

### post_comment_to_feed - post a comment

```json
{"feed_id": "...", "xsec_token": "...", "content": "Great post!"}
```

### reply_comment_in_feed - reply to a comment

```json
{"feed_id": "...", "xsec_token": "...", "content": "Thanks!", "comment_id": "...", "user_id": "..."}
```

`comment_id` and `user_id` come from the comment list returned by get_feed_detail.

### user_profile - user profile

```json
{"user_id": "...", "xsec_token": "..."}
```

`user_id` comes from the feed's `noteCard.user.userId`; use that feed's `xsecToken` as `xsec_token`.

### like_feed - like / unlike

```json
{"feed_id": "...", "xsec_token": "..."}
{"feed_id": "...", "xsec_token": "...", "unlike": true}
```

### favorite_feed - save / unsave

```json
{"feed_id": "...", "xsec_token": "..."}
{"feed_id": "...", "xsec_token": "...", "unfavorite": true}
```

### publish_content - publish an image-text note

```json
{"title": "Title (max 20 chars)", "content": "Body (max 1000 chars)", "images": ["/path/to/img.jpg"], "tags": ["food","travel"]}
```

- `images`: at least 1; local paths or HTTP URLs
- `tags`: optional topic tags
- `schedule_at`: optional scheduled publishing (ISO8601, between 1 hour and 14 days ahead)

### publish_with_video - publish a video

```json
{"title": "Title", "content": "Body", "video": "/path/to/video.mp4"}
```

### Other tools

| Tool | Arguments | Description |
|------|-----------|-------------|
| `check_login_status` | none | Check login status |
| `list_feeds` | none | Get home feed recommendations |
| `get_login_qrcode` | none | Get the login QR code (Base64 PNG) |
| `delete_cookies` | none | Delete cookies and reset login |

## Hot topic tracking

Searches automatically, fetches details and generates a Markdown report.

```bash
./track-topic.sh "DeepSeek" --limit 5
./track-topic.sh "Spring Festival travel" --limit 10 --output report.md
./track-topic.sh "iPhone 16" --limit 5 --feishu    # export to Feishu (needs a feishu-docs skill installed)
```

The report contains overview statistics, hot post details (body + top comments), comment keywords and trend analysis.

## Long image export

Exports posts as a black-on-white JPG long image.

```bash
./export-long-image.sh --posts-file posts.json -o output.jpg
```

posts.json format:
```json
[{
  "title": "Title", "author": "Author", "stats": "13k likes",
  "desc": "Summary", "images": ["https://..."],
  "per_image_text": {"1": "Caption for the 2nd image"}
}]
```

Requires Python 3.10+ and Pillow.

## Notes

- Cookies are valid for about 30 days; scan the QR code again after they expire.
- The first start downloads a headless browser (~150 MB).
- Do not operate the same account from several clients at once.
- Publishing limits: title max 20 characters, body max 1000 characters, at most 50 posts per day.
- Linux servers without a desktop need xvfb (`apt-get install xvfb`; the scripts manage it automatically).
