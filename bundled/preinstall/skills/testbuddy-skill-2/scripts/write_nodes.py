#!/usr/bin/env python3
# -*- coding: utf-8 -*-
# write_nodes.py：统一的节点操作脚本（增删改）
# 用法: echo '[{节点}]' | python3 write_nodes.py add <design_uid>
#       echo '[{节点}]' | python3 write_nodes.py update <design_uid>
#       echo '["uid1"]' | python3 write_nodes.py delete <design_uid>
import json
import sys
import os
import re

# 强制使用 Python 3
if sys.version_info[0] < 3:
    print("错误：此脚本需要 Python 3，请使用 python3 运行", file=sys.stderr)
    sys.exit(1)

# 工作区根目录
WORKSPACE_ROOT = os.getcwd()
TESTBUDDY_DIR = os.path.join(WORKSPACE_ROOT, ".testbuddy", "assets")

def sanitize_json_string(input_str):
    """预处理JSON字符串，修复常见格式问题"""
    input_str = input_str.strip()
    
    # 移除可能的markdown代码块标记
    input_str = re.sub(r'^```json\s*', '', input_str)
    input_str = re.sub(r'^```\s*', '', input_str)
    input_str = re.sub(r'\s*```$', '', input_str)
    
    # 移除注释（// 和 /* */ 风格）
    input_str = re.sub(r'//.*?$', '', input_str, flags=re.MULTILINE)
    input_str = re.sub(r'/\*.*?\*/', '', input_str, flags=re.DOTALL)
    
    return input_str.strip()

def deep_check_json_structure(data, path="root"):
    """深度检查JSON数据结构的合法性"""
    if data is None:
        return True, ""
    
    # 检查循环引用
    if isinstance(data, (dict, list)):
        try:
            json.dumps(data)
        except (TypeError, ValueError) as e:
            return False, "{}: 包含不可序列化的数据 - {}".format(path, str(e))
    
    # 递归检查字典
    if isinstance(data, dict):
        for key, value in data.items():
            if not isinstance(key, str):
                return False, "{}: 字典的键必须是字符串，当前类型：{}".format(path, type(key).__name__)
            
            is_valid, msg = deep_check_json_structure(value, "{}.{}".format(path, key))
            if not is_valid:
                return False, msg
    
    # 递归检查列表
    elif isinstance(data, list):
        for idx, item in enumerate(data):
            is_valid, msg = deep_check_json_structure(item, "{}[{}]".format(path, idx))
            if not is_valid:
                return False, msg
    
    # 检查基本类型
    elif not isinstance(data, (str, int, float, bool)):
        return False, "{}: 不支持的数据类型 {}".format(path, type(data).__name__)
    
    return True, ""

def validate_node_structure(node, index=None):
    """校验单个节点的数据结构"""
    prefix = "节点[{}]".format(index) if index is not None else "节点"
    
    if not isinstance(node, dict):
        return False, "{}: 必须是对象(dict)，当前类型：{}".format(prefix, type(node).__name__)
    
    # 检查必填字段
    required_fields = ["uid", "name", "kind", "parent_uid"]
    for field in required_fields:
        if field not in node:
            return False, "{}: 缺少必填字段 '{}'".format(prefix, field)
    
    # uid 校验
    uid = node["uid"]
    if not isinstance(uid, str) or not uid.strip():
        return False, "{}: uid 必须是非空字符串".format(prefix)
    
    # kind 校验
    valid_kinds = ["STORY", "BUG", "FEATURE", "SCENE", "TEST_POINT", "CASE"]
    if node["kind"] not in valid_kinds:
        return False, "{}: kind 必须是 {} 之一，当前值：{}".format(prefix, valid_kinds, node['kind'])
    
    # 深度检查节点内部结构
    is_valid, msg = deep_check_json_structure(node, prefix)
    if not is_valid:
        return False, msg
    
    return True, ""

def load_update_file(design_uid):
    """加载现有的 update 文件，如果不存在则返回空结构"""
    target_file = os.path.join(TESTBUDDY_DIR, "{}-update.json".format(design_uid))
    if os.path.exists(target_file):
        with open(target_file, 'r', encoding='utf-8') as f:
            return json.load(f)
    else:
        return {"added": [], "updated": [], "deleted": []}

def save_update_file(design_uid, data):
    """保存 update 文件"""
    target_file = os.path.join(TESTBUDDY_DIR, "{}-update.json".format(design_uid))
    target_dir = os.path.dirname(target_file)
    if not os.path.exists(target_dir):
        os.makedirs(target_dir)
    
    with open(target_file, 'w', encoding='utf-8') as f:
        json.dump(data, f, ensure_ascii=False, indent=2)

def handle_add(data, design_uid):
    """处理添加节点操作"""
    # 校验输入必须是数组
    if not isinstance(data, list):
        return {
            "status": "error",
            "msg": "输入必须是节点数组，当前类型：{}".format(type(data).__name__)
        }
    
    # 校验每个节点
    for idx, node in enumerate(data):
        is_valid, msg = validate_node_structure(node, idx)
        if not is_valid:
            return {
                "status": "error",
                "msg": "节点校验失败: {}".format(msg)
            }
    
    # 加载现有文件
    update_data = load_update_file(design_uid)
    
    # 追加到 added 数组
    update_data["added"].extend(data)
    
    # 保存文件
    save_update_file(design_uid, update_data)
    
    target_file = os.path.join(TESTBUDDY_DIR, "{}-update.json".format(design_uid))
    return {
        "status": "success",
        "action": "add",
        "design_uid": design_uid,
        "target_file": target_file,
        "stats": {
            "added": len(data),
            "total_added": len(update_data["added"]),
            "total_updated": len(update_data["updated"]),
            "total_deleted": len(update_data["deleted"])
        }
    }

def handle_update(data, design_uid):
    """处理更新节点操作"""
    # 校验输入必须是数组
    if not isinstance(data, list):
        return {
            "status": "error",
            "msg": "输入必须是节点数组，当前类型：{}".format(type(data).__name__)
        }
    
    # 校验每个节点
    for idx, node in enumerate(data):
        is_valid, msg = validate_node_structure(node, idx)
        if not is_valid:
            return {
                "status": "error",
                "msg": "节点校验失败: {}".format(msg)
            }
    
    # 加载现有文件
    update_data = load_update_file(design_uid)
    
    # 追加到 updated 数组
    update_data["updated"].extend(data)
    
    # 保存文件
    save_update_file(design_uid, update_data)
    
    target_file = os.path.join(TESTBUDDY_DIR, "{}-update.json".format(design_uid))
    return {
        "status": "success",
        "action": "update",
        "design_uid": design_uid,
        "target_file": target_file,
        "stats": {
            "updated": len(data),
            "total_added": len(update_data["added"]),
            "total_updated": len(update_data["updated"]),
            "total_deleted": len(update_data["deleted"])
        }
    }

def handle_delete(data, design_uid):
    """处理删除节点操作"""
    # 校验输入必须是数组
    if not isinstance(data, list):
        return {
            "status": "error",
            "msg": "输入必须是 uid 数组，当前类型：{}".format(type(data).__name__)
        }
    
    # 校验每个 uid
    for idx, uid in enumerate(data):
        if not isinstance(uid, str) or not uid.strip():
            return {
                "status": "error",
                "msg": "[{}]: uid 必须是非空字符串".format(idx)
            }
    
    # 加载现有文件
    update_data = load_update_file(design_uid)
    
    # 追加到 deleted 数组
    update_data["deleted"].extend(data)
    
    # 保存文件
    save_update_file(design_uid, update_data)
    
    target_file = os.path.join(TESTBUDDY_DIR, "{}-update.json".format(design_uid))
    return {
        "status": "success",
        "action": "delete",
        "design_uid": design_uid,
        "target_file": target_file,
        "stats": {
            "deleted": len(data),
            "total_added": len(update_data["added"]),
            "total_updated": len(update_data["updated"]),
            "total_deleted": len(update_data["deleted"])
        }
    }

if __name__ == "__main__":
    # Windows 下设置标准输出编码为 UTF-8
    if sys.platform == 'win32':
        import io
        sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
    
    if len(sys.argv) < 3:
        print(json.dumps({
            "status": "error",
            "msg": "用法: echo '[节点数组]' | python write_nodes.py <action> <design_uid>\n"
                   "  action: add | update | delete\n"
                   "  design_uid: 设计文件的唯一标识"
        }, ensure_ascii=False, indent=2))
        sys.exit(1)
    
    action = sys.argv[1]
    design_uid = sys.argv[2]
    
    # 校验 action 参数
    if action not in ["add", "update", "delete"]:
        print(json.dumps({
            "status": "error",
            "msg": "action 必须是 add/update/delete 之一，当前值：{}".format(action)
        }, ensure_ascii=False, indent=2))
        sys.exit(1)
    
    try:
        # 1. 从标准输入读取 JSON
        input_data = sys.stdin.read()
        
        if not input_data or not input_data.strip():
            print(json.dumps({
                "status": "error",
                "msg": "未接收到输入数据"
            }, ensure_ascii=False, indent=2))
            sys.exit(1)
        
        # 2. 预处理JSON字符串
        input_data = sanitize_json_string(input_data)
        
        # 3. 解析JSON
        try:
            data = json.loads(input_data)
        except json.JSONDecodeError as e:
            error_msg = str(e)
            print(json.dumps({
                "status": "error",
                "msg": "JSON解析失败: {}".format(error_msg)
            }, ensure_ascii=False, indent=2))
            sys.exit(1)
        
        # 4. 根据 action 执行对应操作
        if action == "add" or action == "update":
            # add 和 update 都期望节点数组
            result = handle_add(data, design_uid) if action == "add" else handle_update(data, design_uid)
        elif action == "delete":
            # delete 期望 uid 字符串数组
            result = handle_delete(data, design_uid)
        
        # 5. 输出结果
        print(json.dumps(result, ensure_ascii=False, indent=2))
        sys.exit(0 if result.get("status") == "success" else 1)
    
    except Exception as e:
        print(json.dumps({
            "status": "error",
            "msg": "脚本执行异常: {}".format(str(e))
        }, ensure_ascii=False, indent=2))
        sys.exit(1)
