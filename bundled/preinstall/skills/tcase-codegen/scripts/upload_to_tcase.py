#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
upload_to_tcase.py
TCase 代码上传脚本
用途: 将生成的测试代码上传到 TCase MCP 系统
使用: python3 upload_to_tcase.py --file <文件路径> --repo <仓库名> [选项]

等价替换 upload_to_tcase.sh，去除 jq/curl 依赖。
"""

import argparse
import json
import os
import re
import subprocess
import sys
import urllib.request
import urllib.error
from datetime import datetime

# ===== 路径自发现 =====
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, SCRIPT_DIR)
import env_prepare

# 默认值
DEFAULT_API_URL = "http://tcase.mcp.woa.com/api/v1/upload"


def _ts():
    return datetime.now().strftime("%Y-%m-%d %H:%M:%S")


def print_error(msg):
    print(f"[{_ts()}] [ERROR] {msg}", file=sys.stderr)


def print_success(msg):
    print(f"[{_ts()}] [OK] {msg}")


def print_info(msg):
    print(f"[{_ts()}] [INFO] {msg}")


def print_warning(msg):
    print(f"[{_ts()}] [WARN] {msg}")


def parse_args():
    parser = argparse.ArgumentParser(
        description="TCase 代码上传工具",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""示例:
  # UUID 模式
  python3 %(prog)s --file /tmp/test.py --repo yottadb --case-uuid "uuid-123" --trace-id "trace-abc"

  # Node/Text 模式
  python3 %(prog)s --file /tmp/test.py --repo yottadb --trace-id "trace-xyz"
""",
    )
    parser.add_argument("--file", required=True, help="生成的代码文件路径")
    parser.add_argument("--repo", required=True, help="仓库名称 (如: yottadb)")
    parser.add_argument("--case-uuid", default="", help='测试用例 UUID，多个用逗号分隔 (如: "uuid1,uuid2,uuid3")')
    parser.add_argument("--ai-code-file", default="", help="AI 产出代码的临时文件路径 (由 hook 传入)")
    parser.add_argument("--gen-id", default="", help="generation_id (由 hook 传入)")
    parser.add_argument("--session-id", default="", help="session_id (由 hook 传入)")
    parser.add_argument("--trace-id", default="", help="追踪 ID")
    parser.add_argument("--func-name", default="", help="测试函数名 (由 hook 自动提取)")
    parser.add_argument("--api-url", default=DEFAULT_API_URL, help=f"自定义 API 地址 (默认: {DEFAULT_API_URL})")
    return parser.parse_args()


def extract_function_name(file_path):
    """提取测试函数名（支持多语言）"""
    ext = os.path.splitext(file_path)[1].lstrip(".")

    try:
        with open(file_path, "r", encoding="utf-8", errors="replace") as f:
            content = f.read()
    except Exception:
        return os.path.splitext(os.path.basename(file_path))[0]

    func_name = ""

    if ext == "py":
        m = re.search(r"def (test_[a-zA-Z0-9_]+)", content)
        if m:
            func_name = m.group(1)
    elif ext == "go":
        m = re.search(r"func (Test[a-zA-Z0-9_]+)", content)
        if m:
            func_name = m.group(1)
    elif ext == "java":
        # 优先匹配 @Test 注解后的方法
        m = re.search(r"@Test\s*\n\s*(?:public\s+)?void\s+([a-zA-Z0-9_]+)\s*\(", content)
        if m:
            func_name = m.group(1)
        else:
            m = re.search(r"void\s+(test[a-zA-Z0-9_]+)\s*\(", content)
            if m:
                func_name = m.group(1)
    elif ext in ("cpp", "cc", "cxx", "c"):
        m = re.search(r"TEST(?:_F)?\s*\(\s*([a-zA-Z0-9_]+)\s*,\s*([a-zA-Z0-9_]+)\s*\)", content)
        if m:
            func_name = f"{m.group(1)},{m.group(2)}"
    elif ext in ("js", "ts"):
        m = re.search(r"""(?:it|test)\s*\(\s*['"]([^'"]+)['"]""", content)
        if m:
            func_name = m.group(1)

    # 通用回退
    if not func_name:
        m = re.search(r"(?:def|func|function|void)\s+([Tt]est[a-zA-Z0-9_]*)", content)
        if m:
            func_name = m.group(1)

    # 最终回退：文件名
    if not func_name:
        print("Warning: test function not found, falling back to filename", file=sys.stderr)
        func_name = os.path.splitext(os.path.basename(file_path))[0]

    return func_name


def extract_uuid_from_file(file_path):
    """从文件内容中提取所有 tcase_uuid / design_case_uuid"""
    try:
        with open(file_path, "r", encoding="utf-8", errors="replace") as f:
            content = f.read()
    except Exception:
        return ""

    # 找包含 tcase_uuid 或 design_case_uuid 的行
    tcase_lines = []
    for line in content.splitlines():
        if re.search(r"(?i)tcase_uuid|design_case_uuid", line):
            tcase_lines.append(line)

    if not tcase_lines:
        return ""

    text = "\n".join(tcase_lines)

    # 格式A: RFC4122 标准 UUID（8-4-4-4-12）
    uuids_a = set(re.findall(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", text))
    # 格式B: MCP 新格式（32hex-数字）
    uuids_b = set(re.findall(r"[0-9a-f]{32}-[0-9]+", text))

    all_uuids = sorted(uuids_a | uuids_b)
    return ",".join(all_uuids)


def read_code_content(file_path, ai_code_file):
    """读取 AI 产出的代码内容"""
    if ai_code_file and os.path.isfile(ai_code_file):
        print_info(f"从 AI 代码文件读取: {ai_code_file}")
        with open(ai_code_file, "r", encoding="utf-8", errors="replace") as f:
            return f.read()
    else:
        print_warning(f"未提供 --ai-code-file，降级读取整文件: {file_path}")
        with open(file_path, "r", encoding="utf-8", errors="replace") as f:
            return f.read()


def upload_to_tcase(api_url, username, func_name, code_content, repo_name, file_path, case_uuid, trace_id):
    """上传到 TCase API"""
    print_info("构造上传数据...")

    payload = {
        "ai_code": code_content,
        "req_user": username,
        "case_func_name": func_name,
        "repo_name": repo_name,
        "file_path": file_path,
        "case_uuid": case_uuid,
        "trace_id": trace_id,
    }

    data = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    req = urllib.request.Request(
        api_url,
        data=data,
        headers={"Content-Type": "application/json"},
        method="POST",
    )

    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            http_code = resp.status
            response_body = resp.read().decode("utf-8", errors="replace")
    except urllib.error.HTTPError as e:
        http_code = e.code
        response_body = e.read().decode("utf-8", errors="replace")
    except Exception as e:
        print_error(f"请求失败: {e}")
        return False

    if http_code == 200:
        print_success("上传成功!")

        record_id = "N/A"
        try:
            resp_data = json.loads(response_body)
            record_id = resp_data.get("data", {}).get("id", "N/A")
        except Exception:
            pass

        print("")
        print_success("上传完成!")
        print(f"  文件: {file_path}")
        print(f"  函数: {func_name}")
        print(f"  仓库: {repo_name}")
        print(f"  记录ID: {record_id}")
        if trace_id:
            print(f"  Trace ID: {trace_id}")

        return True
    else:
        print_error(f"上传失败! HTTP {http_code}")
        print("")
        print("响应内容:")
        try:
            print(json.dumps(json.loads(response_body), indent=2, ensure_ascii=False))
        except Exception:
            print(response_body)
        return False


def main():
    print("")
    print_info("========================================")
    print_info("     TCase 代码上传工具")
    print_info("========================================")
    print("")

    args = parse_args()

    file_path = args.file
    repo_name = args.repo
    case_uuid = args.case_uuid
    ai_code_file = args.ai_code_file
    gen_id = args.gen_id
    session_id = args.session_id
    trace_id = args.trace_id
    func_name_arg = args.func_name
    api_url = args.api_url

    # 检查依赖（Python 无需 jq，只需确保 file 存在）
    print_info("检查依赖...")

    # ===== generation_id 校验（session 隔离）=====
    if gen_id and session_id:
        active_gen_file = f"/tmp/pretool_snapshots/{session_id}/.active_gen"
        if os.path.isfile(active_gen_file):
            try:
                active_gen_val = open(active_gen_file, "r").read().strip()
            except Exception:
                active_gen_val = ""
            if active_gen_val != gen_id:
                print_info(f"generation_id 不匹配 (active={active_gen_val}, current={gen_id}), 跳过上传")
                sys.exit(0)
            print_info(f"generation_id 校验通过: {gen_id} (session={session_id})")
        else:
            print_info(f".active_gen 不存在, skill 未激活 (session={session_id}), 跳过上传")
            sys.exit(0)
    elif gen_id:
        print_warning("未提供 --session-id, 跳过 generation 校验")
    else:
        print_warning("未提供 --gen-id, 跳过 generation 校验 (可能是手动调用)")

    # 验证文件
    print_info("验证文件...")
    if not os.path.isfile(file_path):
        print_error(f"文件不存在: {file_path}")
        sys.exit(1)

    # 获取用户名
    print_info("获取用户信息...")
    username = env_prepare.get_username()
    print(f"  用户: {username}")

    # 提取函数名
    print_info("分析代码文件...")
    if func_name_arg:
        func_name = func_name_arg
        print(f"  函数: {func_name} ")
    else:
        func_name = extract_function_name(file_path)
        print(f"  函数: {func_name}")

    # 如果没有提供 CASE_UUID，尝试从文件内容中提取
    if not case_uuid:
        print_info("尝试从文件中提取 tcase_uuid...")
        case_uuid = extract_uuid_from_file(file_path)
        if case_uuid:
            uuid_count = len(case_uuid.split(","))
            print(f"  UUID: {case_uuid} (从文件提取, 共 {uuid_count} 个)")
    else:
        uuid_count = len(case_uuid.split(","))
        print(f"  UUID: {case_uuid} (共 {uuid_count} 个)")

    # 读取代码内容
    print_info("读取代码内容...")
    code_content = read_code_content(file_path, ai_code_file)
    print(f"  长度: {len(code_content)} 字符")

    # 上传
    print("")
    print_info(f"开始上传到 {api_url}")
    print("")

    if upload_to_tcase(api_url, username, func_name, code_content, repo_name, file_path, case_uuid, trace_id):
        print("")
        print_success("任务完成!")
        sys.exit(0)
    else:
        print("")
        print_error("上传失败，请检查日志")
        sys.exit(1)


if __name__ == "__main__":
    main()
