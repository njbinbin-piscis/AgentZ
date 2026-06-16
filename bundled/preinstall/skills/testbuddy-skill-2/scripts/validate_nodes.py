#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
validate_nodes.py：校验节点数据格式
用法: python3 validate_nodes.py <file_path>
支持格式: JSON (.json), YAML (.yaml/.yml), Markdown (.md)
"""
import json
import sys
import os
import re

# 强制使用 Python 3
if sys.version_info[0] < 3:
    print("错误：此脚本需要 Python 3，请使用 python3 运行", file=sys.stderr)
    sys.exit(1)

# 尝试导入 yaml 库
try:
    import yaml
    HAS_YAML = True
except ImportError:
    HAS_YAML = False

def generate_uid(kind):
    """生成节点的唯一ID"""
    import random
    import string
    chars = string.ascii_letters + string.digits
    random_str = ''.join(random.choice(chars) for _ in range(10))
    return "{}-{}".format(kind.lower().replace('_', '_'), random_str)

def parse_yaml_content(content):
    """解析 YAML 内容"""
    if not HAS_YAML:
        return None, "缺少 yaml 库，请安装: pip install pyyaml"
    
    try:
        data = yaml.safe_load(content)
        return data, None
    except Exception as e:
        return None, "YAML 解析失败: {}".format(str(e))

def parse_structured_markdown(content):
    """解析结构化的 Markdown 格式（## 模块，### 场景，#### 测试点，##### 用例）"""
    lines = content.split('\n')
    nodes = []
    current_module = None
    current_scene = None
    current_point = None
    current_case = None
    
    # 用于临时存储当前节点的信息
    temp_data = {}
    in_steps = False
    steps = []
    
    def save_current_case():
        """保存当前用例节点"""
        if current_case and temp_data:
            current_case['instance'] = {
                'preconditions': temp_data.get('前置条件', ''),
                'priority': temp_data.get('优先级', 'P1'),
                'steps': steps
            }
            nodes.append(current_case)
    
    def save_current_point():
        """保存当前测试点节点"""
        if current_point:
            nodes.append(current_point)
    
    def save_current_scene():
        """保存当前场景节点"""
        if current_scene:
            nodes.append(current_scene)
    
    def save_current_module():
        """保存当前模块节点"""
        if current_module:
            nodes.append(current_module)
    
    for line in lines:
        line = line.rstrip()
        
        # 解析标题行
        if line.startswith('## '):
            # 保存之前的所有层级
            save_current_case()
            save_current_point()
            save_current_scene()
            save_current_module()
            
            # 新建模块
            title = line[3:].strip()
            current_module = {
                'name': title,
                'kind': 'FEATURE',
                'description': '',
                'instance': None
            }
            current_scene = None
            current_point = None
            current_case = None
            temp_data = {}
            steps = []
            in_steps = False
            
        elif line.startswith('### '):
            # 保存之前的用例和测试点
            save_current_case()
            save_current_point()
            save_current_scene()
            
            # 新建场景
            title = line[4:].strip()
            current_scene = {
                'name': title,
                'kind': 'SCENE',
                'description': '',
                'instance': None
            }
            current_point = None
            current_case = None
            temp_data = {}
            steps = []
            in_steps = False
            
        elif line.startswith('#### '):
            # 保存之前的用例和测试点
            save_current_case()
            save_current_point()
            
            # 新建测试点
            title = line[5:].strip()
            current_point = {
                'name': title,
                'kind': 'TEST_POINT',
                'description': '',
                'instance': None
            }
            current_case = None
            temp_data = {}
            steps = []
            in_steps = False
            
        elif line.startswith('##### '):
            # 保存之前的用例
            save_current_case()
            
            # 新建用例
            title = line[6:].strip()
            current_case = {
                'name': title,
                'kind': 'CASE',
                'description': '',
                'instance': None
            }
            temp_data = {}
            steps = []
            in_steps = False
        
        # 解析字段
        elif line.startswith('**PARENT_UID：**') or line.startswith('**PARENT_UID:**'):
            match = re.search(r'\*\*PARENT_UID[：:]\*\*\s*(.+)', line)
            if match:
                target = current_case or current_point or current_scene or current_module
                if target:
                    target['parent_uid'] = match.group(1).strip()
        
        elif line.startswith('**UID：**') or line.startswith('**UID:**'):
            match = re.search(r'\*\*UID[：:]\*\*\s*(.+)', line)
            if match:
                target = current_case or current_point or current_scene or current_module
                if target:
                    target['uid'] = match.group(1).strip()
        
        elif line.startswith('**功能描述：**') or line.startswith('**功能描述:**'):
            match = re.search(r'\*\*功能描述[：:]\*\*\s*(.+)', line)
            if match and current_module:
                current_module['description'] = match.group(1).strip()
        
        elif line.startswith('**场景描述：**') or line.startswith('**场景描述:**'):
            match = re.search(r'\*\*场景描述[：:]\*\*\s*(.+)', line)
            if match and current_scene:
                current_scene['description'] = match.group(1).strip()
        
        elif line.startswith('**描述：**') or line.startswith('**描述:**'):
            match = re.search(r'\*\*描述[：:]\*\*\s*(.+)', line)
            if match and current_point:
                current_point['description'] = match.group(1).strip()
        
        elif line.startswith('**用例描述：**') or line.startswith('**用例描述:**'):
            match = re.search(r'\*\*用例描述[：:]\*\*\s*(.+)', line)
            if match and current_case:
                current_case['description'] = match.group(1).strip()
        
        elif line.startswith('**前置条件：**') or line.startswith('**前置条件:**'):
            match = re.search(r'\*\*前置条件[：:]\*\*\s*(.+)', line)
            if match:
                temp_data['前置条件'] = match.group(1).strip()
        
        elif line.startswith('**优先级：**') or line.startswith('**优先级:**'):
            match = re.search(r'\*\*优先级[：:]\*\*\s*(.+)', line)
            if match:
                temp_data['优先级'] = match.group(1).strip()
        
        elif line.startswith('**执行步骤：**') or line.startswith('**执行步骤:**'):
            in_steps = True
            steps = []
        
        elif in_steps and line.startswith('- 步骤'):
            # 解析步骤：- 步骤1：操作描述；预期结果：结果描述
            match = re.search(r'-\s*步骤\d+[：:]\s*(.+?)[；;]\s*预期结果[：:]\s*(.+)', line)
            if match:
                steps.append({
                    'action': match.group(1).strip(),
                    'expected': match.group(2).strip()
                })
        
        elif line.strip() == '' or not line.startswith('-'):
            in_steps = False
    
    # 保存最后剩余的所有节点
    save_current_case()
    save_current_point()
    save_current_scene()
    save_current_module()
    
    # 为没有 uid 的节点生成 uid
    for node in nodes:
        if 'uid' not in node or not node['uid']:
            node['uid'] = generate_uid(node['kind'])
        if 'parent_uid' not in node:
            node['parent_uid'] = None
    
    return nodes

def parse_markdown_content(content):
    """从 Markdown 中提取 YAML/JSON 代码块或解析结构化格式"""
    # 匹配 ```yaml 或 ```json 代码块
    yaml_pattern = r'```(?:yaml|yml)\s*\n(.*?)\n```'
    json_pattern = r'```json\s*\n(.*?)\n```'
    
    yaml_matches = re.findall(yaml_pattern, content, re.DOTALL)
    json_matches = re.findall(json_pattern, content, re.DOTALL)
    
    # 优先尝试 YAML 代码块
    if yaml_matches:
        if not HAS_YAML:
            return None, "缺少 yaml 库，请安装: pip install pyyaml"
        try:
            data = yaml.safe_load(yaml_matches[0])
            return data, None
        except Exception as e:
            return None, "Markdown 中的 YAML 解析失败: {}".format(str(e))
    
    # 尝试 JSON 代码块
    if json_matches:
        try:
            data = json.loads(json_matches[0])
            return data, None
        except Exception as e:
            return None, "Markdown 中的 JSON 解析失败: {}".format(str(e))
    
    # 尝试解析结构化 Markdown 格式
    try:
        nodes = parse_structured_markdown(content)
        if nodes and len(nodes) > 0:
            return nodes, None
    except Exception as e:
        pass
    
    # 如果都不行，尝试直接解析整个内容为 YAML
    if HAS_YAML:
        try:
            data = yaml.safe_load(content)
            if data:
                return data, None
        except:
            pass
    
    return None, "Markdown 文件中未找到有效的 YAML/JSON 代码块或结构化格式"

def flatten_tree_to_list(nodes, parent_uid=None):
    """将树形结构的节点展平为列表，并生成 uid"""
    result = []
    
    if not isinstance(nodes, list):
        nodes = [nodes]
    
    for node in nodes:
        if not isinstance(node, dict):
            continue
        
        # 生成 uid（如果没有）
        if 'uid' not in node:
            node['uid'] = generate_uid(node.get('kind', 'feature'))
        
        # 设置 parent_uid
        if parent_uid:
            node['parent_uid'] = parent_uid
        
        # 确保 instance 字段存在
        if 'instance' not in node:
            node['instance'] = None
        
        # 提取 children
        children = node.pop('children', [])
        
        # 添加当前节点
        result.append(node)
        
        # 递归处理子节点
        if children:
            child_results = flatten_tree_to_list(children, node['uid'])
            result.extend(child_results)
    
    return result

def load_file_data(file_path):
    """根据文件扩展名加载并解析文件"""
    _, ext = os.path.splitext(file_path)
    ext = ext.lower()
    
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # JSON 格式
    if ext == '.json':
        try:
            data = json.loads(content)
            # 如果是树形结构（包含children），展平为列表
            if isinstance(data, (list, dict)):
                data = flatten_tree_to_list(data)
            return data, None
        except Exception as e:
            return None, "JSON 解析失败: {}".format(str(e))
    
    # YAML 格式
    elif ext in ['.yaml', '.yml']:
        data, error = parse_yaml_content(content)
        if error:
            return None, error
        # 如果是树形结构，展平为列表
        if isinstance(data, (list, dict)):
            data = flatten_tree_to_list(data)
        return data, None
    
    # Markdown 格式
    elif ext == '.md':
        data, error = parse_markdown_content(content)
        if error:
            return None, error
        # 如果已经是列表，直接返回（结构化 Markdown 已经是扁平列表）
        if isinstance(data, list):
            return data, None
        # 如果是树形结构，展平为列表
        if isinstance(data, dict):
            data = flatten_tree_to_list(data)
        return data, None
    
    else:
        return None, "不支持的文件格式: {}，支持的格式: .json, .yaml, .yml, .md".format(ext)

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
    
    kind = node["kind"]
    
    # instance 字段校验（仅对 CASE、STORY、BUG 类型强制要求）
    if kind in ["CASE", "STORY", "BUG"]:
        if "instance" not in node:
            return False, "{}: {} 类型缺少必填字段 'instance'".format(prefix, kind)
        instance = node["instance"]
    else:
        # FEATURE/SCENE/TEST_POINT 类型 instance 可以为 null 或不存在
        instance = node.get("instance")
    
    # CASE 类型 instance 校验
    if kind == "CASE":
        if instance is None or not isinstance(instance, dict):
            return False, "{}: CASE 类型的 instance 必须是对象，不能为 null".format(prefix)
        
        # CASE 必填字段
        case_required = ["preconditions", "priority", "steps"]
        for field in case_required:
            if field not in instance:
                return False, "{}: CASE 类型的 instance 缺少必填字段 '{}'".format(prefix, field)
        
        # priority 校验
        valid_priorities = ["P0", "P1", "P2", "P3"]
        if instance["priority"] not in valid_priorities:
            return False, "{}: priority 必须是 {} 之一，当前值：{}".format(prefix, valid_priorities, instance["priority"])
        
        # steps 校验
        steps = instance["steps"]
        if not isinstance(steps, list) or len(steps) == 0:
            return False, "{}: steps 必须是非空数组".format(prefix)
        
        for step_idx, step in enumerate(steps):
            if not isinstance(step, dict):
                return False, "{}: steps[{}] 必须是对象".format(prefix, step_idx)
            if "action" not in step:
                return False, "{}: steps[{}] 缺少必填字段 'action'".format(prefix, step_idx)
            if "expected" not in step:
                return False, "{}: steps[{}] 缺少必填字段 'expected'".format(prefix, step_idx)
    
    # STORY/BUG 类型 instance 校验
    elif kind in ["STORY", "BUG"]:
        if instance is None or not isinstance(instance, dict):
            return False, "{}: {} 类型的 instance 必须是对象，不能为 null".format(prefix, kind)
        
        # STORY/BUG 必填字段
        issue_required = ["workspace", "issue_id"]
        for field in issue_required:
            if field not in instance:
                return False, "{}: {} 类型的 instance 缺少必填字段 '{}'".format(prefix, kind, field)
    
    # 其他类型（FEATURE/SCENE/TEST_POINT）instance 可以为 null
    elif kind in ["FEATURE", "SCENE", "TEST_POINT"]:
        if instance is not None and not isinstance(instance, dict):
            return False, "{}: {} 类型的 instance 必须是 null 或对象".format(prefix, kind)
    
    return True, ""

def validate_nodes(data, file_path):
    """校验节点数据"""
    if not isinstance(data, list):
        return {
            "status": "error",
            "action": "validate",
            "file_path": file_path,
            "msg": "输入必须是节点数组，当前类型：{}".format(type(data).__name__)
        }
    
    if len(data) == 0:
        return {
            "status": "error",
            "action": "validate",
            "file_path": file_path,
            "msg": "节点数组不能为空"
        }
    
    # 校验每个节点
    for idx, node in enumerate(data):
        is_valid, msg = validate_node_structure(node, idx)
        if not is_valid:
            return {
                "status": "error",
                "action": "validate",
                "file_path": file_path,
                "msg": "节点校验失败: {}".format(msg)
            }
    
    return {
        "status": "success",
        "action": "validate",
        "file_path": file_path,
        "stats": {
            "total_nodes": len(data),
            "valid_nodes": len(data)
        },
        "msg": "所有节点校验通过"
    }

if __name__ == "__main__":
    # Windows 下设置标准输出编码为 UTF-8
    if sys.platform == 'win32':
        import io
        sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
    
    if len(sys.argv) < 2:
        print(json.dumps({
            "status": "error",
            "msg": "用法: python validate_nodes.py <file_path>\n"
                   "  file_path: 包含节点数据的文件路径\n"
                   "  支持格式: JSON (.json), YAML (.yaml/.yml), Markdown (.md)"
        }, ensure_ascii=False, indent=2))
        sys.exit(1)
    
    file_path = sys.argv[1]
    
    # 校验文件是否存在
    if not os.path.exists(file_path):
        print(json.dumps({
            "status": "error",
            "action": "validate",
            "msg": "文件不存在: {}".format(file_path)
        }, ensure_ascii=False, indent=2))
        sys.exit(1)
    
    try:
        # 加载并解析文件
        data, error = load_file_data(file_path)
        if error:
            print(json.dumps({
                "status": "error",
                "action": "validate",
                "file_path": file_path,
                "msg": error
            }, ensure_ascii=False, indent=2))
            sys.exit(1)
        
        # 执行校验
        result = validate_nodes(data, file_path)
        
        # 输出结果
        print(json.dumps(result, ensure_ascii=False, indent=2))
        sys.exit(0 if result.get("status") == "success" else 1)
    
    except Exception as e:
        print(json.dumps({
            "status": "error",
            "action": "validate",
            "file_path": file_path,
            "msg": "脚本执行异常: {}".format(str(e))
        }, ensure_ascii=False, indent=2))
        sys.exit(1)
