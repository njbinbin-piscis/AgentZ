#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
task_upload_hook.py
CodeBuddy PostToolUse Hook (matcher: task|Task)

触发时机：子代理（Task）执行完成后
功能：
  1. 从 tool_response.toolInfo 中提取子代理写文件操作的文件路径
  2. 检查文件是否包含 tcase_uuid
  3. 对含 tcase_uuid 的文件调用 upload_to_tcase.py 上传

输入：stdin 接收 JSON（CodeBuddy PostToolUse 事件，tool_name="task"）
输出：stdout 返回 JSON（CodeBuddy Hook 规范）

等价替换 task_upload_hook.sh
"""

import json
import os
import re
import subprocess
import sys
from datetime import datetime

# ===== 路径自发现 =====
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
SKILL_DIR = os.path.dirname(SCRIPT_DIR)

UPLOAD_SCRIPT = os.path.join(SKILL_DIR, "scripts", "upload_to_tcase.py")
LOG_FILE = os.path.join(SKILL_DIR, "logs", "upload.log")
DEBUG_LOG = os.path.join(SKILL_DIR, "logs", "task_hook_debug.log")

# 确保日志目录存在
os.makedirs(os.path.join(SKILL_DIR, "logs"), exist_ok=True)

UUID_RE = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}|[0-9a-f]{32}-[0-9]+")


def _ts():
    return datetime.now().strftime("%Y-%m-%d %H:%M:%S")


def log(msg):
    try:
        with open(DEBUG_LOG, "a", encoding="utf-8") as f:
            f.write(f"[{_ts()}] {msg}\n")
    except Exception:
        pass


def async_work(data):
    """后台异步执行的主逻辑"""
    log("========== Task Hook 被调用 ==========")

    # --- 1. 记录子代理信息 ---
    subagent_name = data.get("tool_input", {}).get("subagent_name", "")
    log(f"subagent_name={subagent_name}")

    # --- 2. 从 toolInfo 中提取写文件操作的文件路径 ---
    tool_response = data.get("tool_response", {})
    if not isinstance(tool_response, dict):
        tool_response = {}
    tool_info_raw = tool_response.get("toolInfo", [])
    log(f"toolInfo 原始数据: {json.dumps(tool_info_raw, ensure_ascii=False)}")

    # 提取所有 Write 相关的 info 字段
    write_infos = []
    if isinstance(tool_info_raw, list):
        for item in tool_info_raw:
            name = item.get("name", "")
            status = item.get("executeStatus", "")
            if re.search(r"(?i)write|edit|replace", name) and status == "completed":
                info = item.get("info", "")
                if info:
                    write_infos.append(info)

    log(f"Write 相关 info: {write_infos}")

    # 从 info 字符串中提取文件路径（绝对路径）
    file_paths = set()
    for info in write_infos:
        paths = re.findall(r"/[^\s]+\.[a-zA-Z0-9]+", info)
        file_paths.update(paths)

    # 如果 toolInfo 没提取到，尝试从 finalResult 中提取
    if not file_paths:
        log("toolInfo 未提取到文件路径，尝试从 finalResult 提取...")
        final_result = tool_response.get("finalResult", "")
        if final_result:
            paths = re.findall(r"/data/workspace/[^\s]+\.[a-zA-Z0-9]+", final_result)
            file_paths.update(paths)

    file_paths = sorted(file_paths)
    log(f"提取到的文件路径: {file_paths}")

    if not file_paths:
        log("跳过: 未找到任何写文件路径")
        return

    # --- 3. 对每个文件检查并上传 ---
    upload_count = 0
    skip_count = 0

    for file_path in file_paths:
        if not file_path:
            continue

        log(f"处理文件: {file_path}")

        # 检查文件存在
        if not os.path.isfile(file_path):
            log(f"跳过: 文件不存在 {file_path}")
            skip_count += 1
            continue

        # 检查文件是否包含 tcase_uuid 或 design_case_uuid
        try:
            with open(file_path, "r", encoding="utf-8", errors="replace") as f:
                content = f.read()
        except Exception:
            log(f"跳过: 无法读取文件 {file_path}")
            skip_count += 1
            continue

        if not re.search(r"tcase_uuid|design_case_uuid", content):
            log(f"跳过: 文件不含 tcase_uuid 或 design_case_uuid {file_path}")
            skip_count += 1
            continue

        # 提取 UUID
        tcase_lines = "\n".join(
            line for line in content.splitlines()
            if re.search(r"(?i)tcase_uuid|design_case_uuid", line)
        )
        all_uuids = set(UUID_RE.findall(tcase_lines)) if tcase_lines else set()
        all_uuids_str = ",".join(sorted(all_uuids))

        log(f"提取到 UUID: {all_uuids_str}")

        # 推断 repo_name
        repo_name = ""
        if "/yottadb/" in file_path:
            repo_name = "yottadb"
        elif "/yp/" in file_path:
            repo_name = "yp"
        else:
            # 回退：用 workspace 下第一级目录名
            m = re.match(r"^/data/workspace/([^/]+)", file_path)
            if m:
                repo_name = m.group(1)

        log(f"开始上传: file={file_path}, repo={repo_name or 'unknown'}, uuids={all_uuids_str}")

        # 执行上传
        cmd = [
            sys.executable, UPLOAD_SCRIPT,
            "--file", file_path,
            "--repo", repo_name or "unknown",
            "--case-uuid", all_uuids_str,
        ]

        try:
            with open(LOG_FILE, "a", encoding="utf-8") as lf:
                result = subprocess.run(cmd, stdout=lf, stderr=lf, timeout=60)
            exit_code = result.returncode
        except Exception as e:
            log(f"上传脚本执行异常: {e}")
            exit_code = 1

        log(f"上传完成: file={file_path}, exit={exit_code}")

        if exit_code == 0:
            upload_count += 1

    log(f"Task Hook 执行完毕: 上传={upload_count}, 跳过={skip_count}")


def main():
    # ===== 读取 stdin JSON =====
    raw_input = sys.stdin.read()

    # 立即返回，不阻塞主流程
    print('{"continue": true}')
    sys.stdout.flush()

    # 解析 JSON
    try:
        data = json.loads(raw_input)
    except Exception:
        data = {}

    # ===== 以下逻辑在后台异步执行 =====
    pid = os.fork()
    if pid > 0:
        # 父进程立即退出
        sys.exit(0)
    else:
        # 子进程：关闭 stdin/stdout 避免污染 Hook 协议
        try:
            sys.stdin.close()
            devnull = open(os.devnull, "w")
            sys.stdout = devnull
            sys.stderr = devnull
        except Exception:
            pass

        try:
            async_work(data)
        except Exception as e:
            log(f"异步工作异常: {e}")
        finally:
            os._exit(0)


if __name__ == "__main__":
    main()
