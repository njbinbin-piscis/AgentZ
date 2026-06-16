#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
pre_tool_debug.py
PreToolUse Hook
职责：
  1. Bash + env_prepare 命令 → 写 .active 到 session 隔离目录
  2. Write/Edit 命令 → 管理快照（session 隔离）
  3. session/generation 级别清理

等价替换 pre_tool_debug.sh
"""

import hashlib
import json
import os
import shutil
import sys
import time
from datetime import datetime
from pathlib import Path

# ===== 路径自发现 =====
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
SKILL_DIR = os.path.dirname(SCRIPT_DIR)

DEBUG_LOG = os.path.join(SKILL_DIR, "logs", "write_hook_debug.log")
BASE_DIR = "/tmp/pretool_snapshots"

os.makedirs(os.path.dirname(DEBUG_LOG), exist_ok=True)
os.makedirs(BASE_DIR, exist_ok=True)

TAG = "[PreToolUse]"


def log(msg):
    """写日志到 debug log 文件"""
    ts = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    try:
        with open(DEBUG_LOG, "a", encoding="utf-8") as f:
            f.write(f"[{ts}] {TAG} {msg}\n")
    except Exception:
        pass


def main():
    # 读取 stdin JSON
    raw_input = sys.stdin.read()

    log("===== PreToolUse 触发 =====")

    # 解析 JSON
    try:
        data = json.loads(raw_input)
    except Exception:
        data = {}

    # 记录完整 INPUT JSON
    log("===== 完整 INPUT JSON 开始 =====")
    try:
        with open(DEBUG_LOG, "a", encoding="utf-8") as f:
            f.write(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
    except Exception:
        try:
            with open(DEBUG_LOG, "a", encoding="utf-8") as f:
                f.write(raw_input + "\n")
        except Exception:
            pass
    log("===== 完整 INPUT JSON 结束 =====")

    # 提取公共字段
    tool_name = data.get("tool_name", "")
    tool_input = data.get("tool_input", {})
    if not isinstance(tool_input, dict):
        tool_input = {}
    file_path = tool_input.get("filePath", "") or tool_input.get("file_path", "")
    gen_id = data.get("generation_id", "")
    session_id = data.get("session_id", "")

    log(f"tool_name: {tool_name}, filePath: {file_path}, generation_id: {gen_id}, session_id: {session_id}")

    # session 隔离目录
    session_dir = os.path.join(BASE_DIR, session_id) if session_id else BASE_DIR
    os.makedirs(session_dir, exist_ok=True)

    # ===== Bash/execute_command 工具：检测 env_prepare 命令，写 .active =====
    if tool_name in ("Bash", "execute_command"):
        command = data.get("tool_input", {}).get("command", "")
        log(f"Bash 工具, command: {command}")

        if "env_prepare" in command:
            active_marker = os.path.join(session_dir, ".active")
            active_gen_file = os.path.join(session_dir, ".active_gen")
            log(f"检测到 env_prepare 命令, 写入 {active_marker}, 清除旧 .active_gen")
            Path(active_marker).touch()
            try:
                os.remove(active_gen_file)
            except FileNotFoundError:
                pass

        print('{"continue": true}')
        sys.exit(0)

    # ===== 以下仅处理 Write/Edit 工具 =====

    # ===== session 级别清理：清理其他 session 的残留目录 =====
    try:
        for entry in os.listdir(BASE_DIR):
            dir_path = os.path.join(BASE_DIR, entry)
            if not os.path.isdir(dir_path):
                continue
            if entry == session_id:
                continue
            # 检查目录修改时间，超过 24 小时才清理
            try:
                mtime = os.path.getmtime(dir_path)
                if (time.time() - mtime) > 86400:  # 24 * 60 * 60
                    log(f"清理过期 session 目录: {entry}")
                    shutil.rmtree(dir_path, ignore_errors=True)
            except Exception:
                pass
    except Exception:
        pass

    # ===== generation 级别清理：清除残留 .active =====
    active_marker = os.path.join(session_dir, ".active")
    active_gen_file = os.path.join(session_dir, ".active_gen")
    if os.path.isfile(active_marker) and gen_id and os.path.isfile(active_gen_file):
        try:
            active_gen_val = open(active_gen_file, "r").read().strip()
            if active_gen_val != gen_id:
                log(f"generation 变更, 清除残留 .active (active_gen={active_gen_val}, cur_gen={gen_id})")
                os.remove(active_marker)
        except Exception:
            pass

    # ===== 快照管理（session 隔离目录下） =====
    if file_path and os.path.isfile(file_path):
        try:
            with open(file_path, "r", encoding="utf-8", errors="replace") as f:
                lines = f.readlines()
            line_count = len(lines)
            log(f"修改前文件行数: {line_count}")
        except Exception:
            line_count = 0
            log("读取文件行数失败")

        file_key = hashlib.md5(file_path.encode()).hexdigest()
        snapshot = os.path.join(session_dir, file_key)
        gen_file = snapshot + ".gen"

        old_gen_id = ""
        if os.path.isfile(gen_file):
            try:
                old_gen_id = open(gen_file, "r").read().strip()
            except Exception:
                pass

        if os.path.isfile(snapshot) and os.path.isfile(gen_file) and gen_id and old_gen_id == gen_id:
            log(f"同一 generation，快照已存在，跳过: {snapshot} (gen={gen_id})")
        else:
            if os.path.isfile(snapshot):
                log(f"generation_id 变更或标记不一致，重建快照 (old_gen={old_gen_id or '<无>'}, new_gen={gen_id or '<无>'})")
                try:
                    os.remove(snapshot)
                except Exception:
                    pass
                try:
                    os.remove(gen_file)
                except Exception:
                    pass
            try:
                shutil.copy2(file_path, snapshot)
                with open(gen_file, "w") as f:
                    f.write(gen_id)
                log(f"已存快照: {snapshot} (gen={gen_id})")
            except Exception as e:
                log(f"快照保存失败: {e}")
    else:
        log("文件不存在（新建文件场景）")

    try:
        with open(DEBUG_LOG, "a", encoding="utf-8") as f:
            f.write("\n")
    except Exception:
        pass

    print('{"continue": true}')
    sys.exit(0)


if __name__ == "__main__":
    main()
