#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
tcase_upload_hook.py
CodeBuddy PostToolUse Hook

触发时机：write_to_file / replace_in_file 执行完成后
上传条件：文件内容包含 tcase_uuid（不限语言）
输入：stdin 接收 JSON（CodeBuddy PostToolUse 事件）
输出：stdout 返回 JSON（CodeBuddy Hook 规范）

等价替换 tcase_upload_hook.sh
"""

import hashlib
import json
import os
import re
import subprocess
import sys
import time
from datetime import datetime

# ===== 路径自发现 =====
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
SKILL_DIR = os.path.dirname(SCRIPT_DIR)

UPLOAD_SCRIPT = os.path.join(SKILL_DIR, "scripts", "upload_to_tcase.py")
LOG_FILE = os.path.join(SKILL_DIR, "logs", "upload.log")
DEBUG_LOG = os.path.join(SKILL_DIR, "logs", "write_hook_debug.log")

# 确保日志目录存在
os.makedirs(os.path.join(SKILL_DIR, "logs"), exist_ok=True)

TAG = "[PostToolUse]"
UUID_RE = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}|[0-9a-f]{32}-[0-9]+")


def log(msg):
    ts = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    try:
        with open(DEBUG_LOG, "a", encoding="utf-8") as f:
            f.write(f"[{ts}] {TAG} {msg}\n")
    except Exception:
        pass


def extract_uuids_from_text(text):
    """从给定文本中提取所有 UUID，去重后逗号拼接"""
    uuids = set(UUID_RE.findall(text))
    return ",".join(sorted(uuids))


def diff_extract_added_lines(snapshot_path, current_content):
    """用逐行比较提取新增行内容（替代 diff 命令的 '>' 行）"""
    try:
        with open(snapshot_path, "r", encoding="utf-8", errors="replace") as f:
            old_lines = f.readlines()
    except Exception:
        return ""

    new_lines = current_content.splitlines(keepends=True)

    # 简单 diff：用集合差异可能不够精确，用 difflib
    import difflib
    differ = difflib.Differ()
    diff_result = list(differ.compare(old_lines, new_lines))

    added = []
    for line in diff_result:
        if line.startswith("+ "):
            added.append(line[2:])
        elif line.startswith("+") and len(line) > 1 and line[1] != "+":
            added.append(line[2:])

    return "".join(added)


def diff_get_changed_line_numbers(snapshot_path, current_content):
    """获取改动行的行号列表（1-based，对应新文件）"""
    try:
        with open(snapshot_path, "r", encoding="utf-8", errors="replace") as f:
            old_lines = f.read().splitlines()
    except Exception:
        return []

    new_lines = current_content.splitlines()

    import difflib
    sm = difflib.SequenceMatcher(None, old_lines, new_lines)
    changed_lines = []

    for tag, i1, i2, j1, j2 in sm.get_opcodes():
        if tag in ("replace", "insert"):
            for ln in range(j1 + 1, j2 + 1):  # 1-based
                changed_lines.append(ln)

    return changed_lines


def backtrace_uuids(file_lines, start_line_0based):
    """从指定行号向上回溯提取 UUID
    两阶段: 1) 跳过无 UUID 的行直到找到第一个有 UUID 的行
            2) 连续收集 UUID 直到遇到无 UUID 的行
    """
    result = []
    found_first = False

    for ln in range(start_line_0based, -1, -1):
        line = file_lines[ln]
        line_uuids = UUID_RE.findall(line)

        if line_uuids:
            found_first = True
            result.extend(line_uuids)
        else:
            if found_first:
                break
            # 还没找到第一个有 UUID 的行，继续跳过

    return result


def async_work(data, sync_file_path, sync_fk):
    """后台异步执行的主逻辑"""
    log(f"Hook 被调用, PID={os.getpid()}")

    # 记录完整的 INPUT JSON
    log("===== 完整 INPUT JSON 开始 =====")
    try:
        with open(DEBUG_LOG, "a", encoding="utf-8") as f:
            f.write(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
    except Exception:
        pass
    log("===== 完整 INPUT JSON 结束 =====")

    file_path = sync_file_path
    cwd = data.get("cwd", "")
    cur_gen_id = data.get("generation_id", "")
    cur_session_id = data.get("session_id", "")

    # session 隔离目录
    base_dir = "/tmp/pretool_snapshots"
    session_dir = os.path.join(base_dir, cur_session_id) if cur_session_id else base_dir

    log(f"FILE_PATH={file_path}, CWD={cwd}, session={cur_session_id}")

    # 条件 1：文件路径有效且文件存在
    if not file_path or not os.path.isfile(file_path):
        log("跳过: 文件不存在或路径为空")
        return

    # ===== 条件 1.5：检查 tcase-codegen skill 激活标记（session 隔离）=====
    active_marker = os.path.join(session_dir, ".active")
    active_gen_file = os.path.join(session_dir, ".active_gen")

    if os.path.isfile(active_marker):
        log(f"检测到 {active_marker}, 写入 .active_gen={cur_gen_id}, 删除 .active")
        try:
            with open(active_gen_file, "w") as f:
                f.write(cur_gen_id)
        except Exception:
            pass
        try:
            os.remove(active_marker)
        except Exception:
            pass

    # 判断当前 generation 是否匹配
    if not os.path.isfile(active_gen_file):
        log(f"跳过: .active_gen 不存在, tcase-codegen skill 未激活 (session={cur_session_id})")
        return

    try:
        active_gen_val = open(active_gen_file, "r").read().strip()
    except Exception:
        active_gen_val = ""

    if active_gen_val != cur_gen_id:
        log(f"跳过: generation_id 不匹配 (active_gen={active_gen_val}, cur_gen={cur_gen_id})")
        return

    log(f"generation_id 匹配 ({cur_gen_id}), 继续处理")

    # ★ 提取 tool_response 里的统计信息
    tool_resp = data.get("tool_response", {})
    if not isinstance(tool_resp, dict):
        tool_resp = {}
    add_lines = tool_resp.get("addLineCount", 0)
    del_lines = tool_resp.get("removedLines", 0)
    add_chars = tool_resp.get("addedChars", 0)
    del_chars = tool_resp.get("removedChars", 0)
    is_new = tool_resp.get("isNewFile", False)
    # is_new 可能是字符串 "true"/"false" 或布尔值
    if isinstance(is_new, str):
        is_new = is_new.lower() == "true"

    log(f"tool_response统计: 新增{add_lines}行, 删除{del_lines}行, 新增{add_chars}字符, 删除{del_chars}字符, 新文件={is_new}")

    # 定位原始快照（session 隔离目录下）
    file_key = sync_fk
    snapshot = os.path.join(session_dir, file_key) if file_key else ""
    if file_key and os.path.isfile(snapshot):
        log(f"找到原始快照: {snapshot}")
    else:
        snapshot = ""
        log("无原始快照（新建文件，或 PreToolUse 未触发）")

    # 等待文件写入磁盘完成，最多 MAX_WAIT * 0.5 秒
    try:
        with open(file_path, "rb") as f:
            initial_md5 = hashlib.md5(f.read()).hexdigest()
    except Exception:
        initial_md5 = ""

    max_wait = 10
    waited = 0
    while waited < max_wait:
        time.sleep(0.5)
        try:
            with open(file_path, "rb") as f:
                current_md5 = hashlib.md5(f.read()).hexdigest()
        except Exception:
            current_md5 = initial_md5
        if current_md5 != initial_md5:
            log(f"文件已更新 (等待了{waited + 1}x0.5s), md5: {initial_md5} -> {current_md5}")
            break
        waited += 1

    if waited >= max_wait:
        log(f"文件md5未变化 (等待了{max_wait}x0.5s), 可能是write_to_file新建文件, 继续执行")

    # 落盘后读取修改后的文件内容
    try:
        with open(file_path, "r", encoding="utf-8", errors="replace") as f:
            after_content = f.read()
    except Exception:
        after_content = ""

    after_lines = len(after_content.splitlines())
    log(f"修改后文件行数: {after_lines}")

    # 提取 AI 本次产出的代码行，写入临时文件供上传使用
    ai_code_file = os.path.join(session_dir, f"{file_key}_ai_code_{os.getpid()}") if file_key else ""

    log("===== AI本次写入/修改的代码 =====")
    try:
        if is_new:
            log("(新增文件，全部为AI生成)")
            if ai_code_file:
                with open(ai_code_file, "w", encoding="utf-8") as f:
                    f.write(after_content)
            with open(DEBUG_LOG, "a", encoding="utf-8") as f:
                f.write(after_content + "\n")
        elif snapshot and os.path.isfile(snapshot):
            ai_code = diff_extract_added_lines(snapshot, after_content)
            if ai_code.strip():
                if ai_code_file:
                    with open(ai_code_file, "w", encoding="utf-8") as f:
                        f.write(ai_code)
                with open(DEBUG_LOG, "a", encoding="utf-8") as f:
                    f.write(ai_code + "\n")
            else:
                log("(与原始快照无差异，降级使用整文件)")
                if ai_code_file:
                    with open(ai_code_file, "w", encoding="utf-8") as f:
                        f.write(after_content)
                with open(DEBUG_LOG, "a", encoding="utf-8") as f:
                    f.write("(无变更)\n")
        else:
            log("(无修改前快照，降级输出全文件)")
            if ai_code_file:
                with open(ai_code_file, "w", encoding="utf-8") as f:
                    f.write(after_content)
            with open(DEBUG_LOG, "a", encoding="utf-8") as f:
                f.write(after_content + "\n")
    except Exception as e:
        log(f"AI 代码提取异常: {e}")

    log("===== AI代码结束 =====")
    try:
        with open(DEBUG_LOG, "a", encoding="utf-8") as f:
            f.write("\n")
    except Exception:
        pass

    # 条件 2：文件内容包含 tcase_uuid 或 design_case_uuid 标识
    if not re.search(r"tcase_uuid|design_case_uuid", after_content):
        log("跳过: 文件不含 tcase_uuid 或 design_case_uuid")
        _cleanup(ai_code_file)
        return

    log("条件全部通过，开始提取 UUID")

    # ===== UUID 提取 =====
    all_uuids_set = set()
    file_lines = after_content.splitlines()

    if is_new:
        # 新建文件：提取全文件所有 UUID
        log("新建文件, 提取整文件所有 UUID")
        tcase_lines = "\n".join(
            line for line in file_lines if re.search(r"(?i)tcase_uuid|design_case_uuid", line)
        )
        all_uuids_set = set(UUID_RE.findall(tcase_lines))

    elif snapshot and os.path.isfile(snapshot):
        # 有快照：离散代码块 + 块内提取 + 向上回溯
        log("使用离散代码块 + 向上回溯提取 UUID")

        changed_line_nums = diff_get_changed_line_numbers(snapshot, after_content)

        if changed_line_nums:
            log(f"diff 检测到改动行号: {changed_line_nums}")

            # 合并连续行号为离散代码块
            blocks = []  # [(start, end), ...] 1-based
            block_start = changed_line_nums[0]
            prev_num = changed_line_nums[0]

            for ln in changed_line_nums[1:]:
                if ln - prev_num > 1:
                    blocks.append((block_start, prev_num))
                    block_start = ln
                prev_num = ln
            blocks.append((block_start, prev_num))

            log(f"离散代码块: {len(blocks)} 个")

            # 对每个离散代码块提取 UUID
            for i, (b_start, b_end) in enumerate(blocks):
                block_uuids = set()

                # 步骤1: 块内提取 UUID (行号是1-based, 数组是0-based)
                for ln in range(b_start - 1, min(b_end, len(file_lines))):
                    line_uuids = UUID_RE.findall(file_lines[ln])
                    block_uuids.update(line_uuids)

                # 步骤2: 从块第一行向上回溯
                backtrace_uuids_result = set()
                if b_start - 2 >= 0:
                    backtrace_uuids_result = set(backtrace_uuids(file_lines, b_start - 2))

                all_uuids_set.update(block_uuids)
                all_uuids_set.update(backtrace_uuids_result)

            if not all_uuids_set:
                log("所有代码块均未关联到 UUID, fallback 全文件 UUID")
                tcase_lines = "\n".join(
                    line for line in file_lines if re.search(r"(?i)tcase_uuid|design_case_uuid", line)
                )
                all_uuids_set = set(UUID_RE.findall(tcase_lines))
        else:
            log("diff 无改动行（文件与快照相同）, fallback 提取整文件所有 UUID")
            tcase_lines = "\n".join(
                line for line in file_lines if re.search(r"(?i)tcase_uuid|design_case_uuid", line)
            )
            all_uuids_set = set(UUID_RE.findall(tcase_lines))
    else:
        # 无快照：提取全文件所有 UUID
        log("无快照, 提取整文件所有 UUID")
        tcase_lines = "\n".join(
            line for line in file_lines if re.search(r"(?i)tcase_uuid|design_case_uuid", line)
        )
        all_uuids_set = set(UUID_RE.findall(tcase_lines))

    all_uuids = ",".join(sorted(all_uuids_set))
    log(f"提取到 ALL_UUIDS='{all_uuids}'")

    # 从 cwd 推断 repo_name
    repo_name = os.path.basename(cwd) if cwd else ""

    # 执行上传
    log(f"开始上传: file={file_path}, repo={repo_name or 'unknown'}, uuids={all_uuids}, gen_id={cur_gen_id}, session={cur_session_id}, ai_code_file={ai_code_file}")

    cmd = [
        sys.executable, UPLOAD_SCRIPT,
        "--file", file_path,
        "--repo", repo_name or "unknown",
        "--case-uuid", all_uuids,
    ]
    if ai_code_file and os.path.isfile(ai_code_file):
        cmd.extend(["--ai-code-file", ai_code_file])
    if cur_gen_id:
        cmd.extend(["--gen-id", cur_gen_id])
    if cur_session_id:
        cmd.extend(["--session-id", cur_session_id])

    try:
        with open(LOG_FILE, "a", encoding="utf-8") as lf:
            subprocess.run(cmd, stdout=lf, stderr=lf, timeout=60)
    except Exception as e:
        log(f"上传脚本执行异常: {e}")

    upload_exit = 0  # 简化处理
    log(f"上传完成, exit={upload_exit}")

    # 清理 AI 代码临时文件
    _cleanup(ai_code_file)


def _cleanup(ai_code_file):
    if ai_code_file:
        try:
            os.remove(ai_code_file)
        except Exception:
            pass


def main():
    # ===== 读取 stdin JSON =====
    raw_input = sys.stdin.read()

    # ===== 同步阶段：解析 FILE_PATH =====
    try:
        data = json.loads(raw_input)
    except Exception:
        data = {}

    tool_input = data.get("tool_input", {})
    sync_file_path = tool_input.get("filePath", "") or tool_input.get("file_path", "")
    if not sync_file_path:
        tr = data.get("tool_response", {})
        if isinstance(tr, dict):
            sync_file_path = tr.get("path", "")

    # 计算 FILE_KEY
    sync_fk = hashlib.md5(sync_file_path.encode()).hexdigest() if sync_file_path else ""

    # 立即返回，不阻塞主流程
    print('{"continue": true}')
    sys.stdout.flush()

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
            async_work(data, sync_file_path, sync_fk)
        except Exception as e:
            log(f"异步工作异常: {e}")
        finally:
            os._exit(0)


if __name__ == "__main__":
    main()
