# load_session

加载当前测试会话信息，获取 design_uid、workspace_id 等参数。

## 使用方式

```bash
python3 <skills_dir>/scripts/load_session.py
```

**注意**：执行脚本前**不要切换目录（不要 cd）**，确保在工作区根目录执行。

## 注意事项

- <skills_dir>是脚本所在路径的前缀
- **脚本依赖当前工作目录，执行前不要 cd 切换目录**

## 输出示例

```json
{
  "design_uid": "design-Az7SsiL3Ui",
  "namespace": "...",
  "select_node": {
    "uid": "test_point-X0krRg3bP2",
    "kind": "TEST_POINT",
    "name": "搜索结果展示"
  },
  "story_node": {
    "instance": {
      "WorkspaceUid": "69995517"
    }
  }
}
```
