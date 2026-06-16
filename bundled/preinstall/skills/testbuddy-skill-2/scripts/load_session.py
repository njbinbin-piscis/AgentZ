#!/usr/bin/env python3
# -*- coding: utf-8 -*-
import json
import os
import sys

# 强制使用 Python 3
if sys.version_info[0] < 3:
    print("错误：此脚本需要 Python 3，请使用 python3 运行", file=sys.stderr)
    sys.exit(1)

# 固定路径
SESSION_FILE = os.path.join(os.getcwd(), ".testbuddy/env/session.json")

def load_session():
    """加载 session.json 文件"""
    if not os.path.exists(SESSION_FILE):
        return {}
    
    with open(SESSION_FILE, 'r', encoding='utf-8') as f:
        return json.load(f)

if __name__ == "__main__":
    data = load_session()
    print(json.dumps(data, ensure_ascii=False, indent=2))
