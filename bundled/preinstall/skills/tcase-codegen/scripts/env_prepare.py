#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
env_prepare.py
公共工具模块：提供 get_username()、scan_git_repos() 等公共函数。
直接执行时输出 JSON 格式的环境信息（用户名、Git 仓库列表）。
被其他脚本 import 时仅提供函数，不执行 Git 扫描。

等价替换 env_prepare.sh，去除 jq 依赖。
"""

import json
import os
import subprocess
import sys


def get_username():
    """获取当前用户名，优先级：git config user.name > global > email > whoami > 默认值"""
    # 1. git config user.name (local)
    username = _git_config("user.name")
    if username:
        return username

    # 2. git config --global user.name
    username = _git_config_global("user.name")
    if username:
        return username

    # 3. git config user.email 取 @ 前面
    email = _git_config("user.email")
    if email and "@" in email:
        return email.split("@")[0]

    # 4. whoami，排除 root
    try:
        u = subprocess.run(["whoami"], capture_output=True, text=True, timeout=5).stdout.strip()
        if u and u != "root":
            return u
    except Exception:
        pass

    return "TCase user"


def scan_git_repos(search_dir):
    """扫描 search_dir 下最多2层的 Git 仓库，返回仓库信息列表。

    Returns:
        list[dict]: 每个元素包含 name, path, branch
    """
    repos = []
    try:
        result = subprocess.run(
            ["find", search_dir, "-maxdepth", "2", "-name", ".git", "-type", "d"],
            capture_output=True, text=True, timeout=10,
        )
        git_dirs = [d.strip() for d in result.stdout.strip().split("\n") if d.strip()]
    except Exception:
        git_dirs = []

    for git_dir in git_dirs:
        repo_path = os.path.dirname(git_dir)

        # 获取仓库名：优先从 remote url 提取
        repo_name = _get_repo_name(repo_path)

        # 获取分支
        branch = _get_branch(repo_path)

        repos.append({
            "name": repo_name,
            "path": repo_path,
            "branch": branch,
        })

    return repos


# ===== 内部辅助函数 =====

def _git_config(key):
    """获取 git config 值（local）"""
    try:
        r = subprocess.run(["git", "config", key], capture_output=True, text=True, timeout=5)
        return r.stdout.strip() if r.returncode == 0 else ""
    except Exception:
        return ""


def _git_config_global(key):
    """获取 git config --global 值"""
    try:
        r = subprocess.run(["git", "config", "--global", key], capture_output=True, text=True, timeout=5)
        return r.stdout.strip() if r.returncode == 0 else ""
    except Exception:
        return ""


def _get_repo_name(repo_path):
    """从 git remote 获取仓库名，回退用目录名"""
    try:
        r = subprocess.run(
            ["git", "-C", repo_path, "remote", "get-url", "origin"],
            capture_output=True, text=True, timeout=5,
        )
        remote_url = r.stdout.strip()
        if remote_url:
            name = os.path.basename(remote_url)
            if name.endswith(".git"):
                name = name[:-4]
            return name
    except Exception:
        pass
    return os.path.basename(repo_path)


def _get_branch(repo_path):
    """获取 Git 当前分支"""
    try:
        r = subprocess.run(
            ["git", "-C", repo_path, "branch", "--show-current"],
            capture_output=True, text=True, timeout=5,
        )
        branch = r.stdout.strip()
        return branch if branch else "master"
    except Exception:
        return "master"


def main():
    """直接执行时：扫描 Git 仓库并输出 JSON"""
    search_dir = sys.argv[1] if len(sys.argv) > 1 else os.getcwd()

    user_name = get_username()
    repos = scan_git_repos(search_dir)

    output = {
        "user_id": user_name,
        "search_dir": search_dir,
        "repos": repos,
        "repo_count": len(repos),
    }

    print(json.dumps(output, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
