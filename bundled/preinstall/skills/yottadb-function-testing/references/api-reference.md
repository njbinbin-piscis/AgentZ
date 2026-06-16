# API 参考索引

本文档提供 YottaDB 测试项目中常用 API 的完整参考。

> **重要**：使用任何 API 前，请通过 `grep` 命令确认其存在性和参数签名。

## 一、API 检索方法

### 1.1 检索 DAO 方法

```bash
# 检索系统管理 DAO
grep "def " test/dao/sys_admin_dao.py

# 检索用户管理 DAO
grep "def " test/dao/user_admin_dao.py

# 检索数据管理 DAO
grep "def " test/dao/data_admin_dao.py

# 检索控制面入口
grep "def " test/dao/controlplane.py
```

### 1.2 检索基类方法

```bash
# 检索 TSDB 基类
grep "def " test/case/tsdb/test_tsdb_base.py

# 检索数据面基类
grep "def " test/case/data_plane/test_cluster_case_base.py

# 检索系统基类
grep "def " test/case/system_case.py
```

## 二、SystemAdminDao API

**文件位置**：`test/dao/sys_admin_dao.py`

### 2.1 账户管理

```python
# 获取账户 ID（不需要 app_id）
account_id = self.sys_admin.get_account_id_by_name_without_app_id(account_name)
# 返回: str，账户 ID，不存在返回空字符串

# 获取账户元数据
rsp = self.sys_admin.get_account_meta(account_id)
# 返回: GetAccountMetaResponse
#   - rsp.err_code: 错误码
#   - rsp.account: Account 消息

# 列出所有账户
rsp = self.sys_admin.list_accounts(app_id)
# 返回: ListAccountsResponse
#   - rsp.account_metas: List[Account]
```

### 2.2 数据库管理

```python
# 列出数据库
rsp = self.sys_admin.list_databases(account_id)
# 返回: ListDatabasesResponse
#   - rsp.err_code: 错误码
#   - rsp.database_metas: List[Database]

# 获取数据库元数据
rsp = self.sys_admin.get_database_meta(database_id)
```

### 2.3 Collection 管理

```python
# 列出 Collection
rsp = self.sys_admin.list_collection_metas(database_id)
# 返回: ListCollectionMetasResponse
#   - rsp.err_code: 错误码
#   - rsp.collection_metas: List[Collection]

# 获取 Collection 元数据
rsp = self.sys_admin.get_collection_meta(collection_id)
```

### 2.4 SubCluster 管理

```python
# 列出 SubCluster Group
rsp = self.sys_admin.list_sub_cluster_group(cluster_id=self.cluster_id)
# 返回: ListSubClusterGroupResponse
#   - rsp.err_code: 错误码
#   - rsp.sub_cluster_groups: List[SubClusterGroup]
#       - scg.sub_clusters: List[SubCluster]
#           - sc.base_info._rid: SC ID
#           - sc.base_info.name: SC 名称
#           - sc.alloc_affinity: 分配亲和性

# 更新 SC 分配亲和性
rsp = self.sys_admin.update_sc_alloc_affinity(
    sub_cluster_ids=[sc_id_1, sc_id_2],  # SC ID 列表
    alloc_affinity=new_affinity,          # 新亲和性
    ignore_existed_data=False             # 是否忽略已存在数据
)
# 返回: UpdateScAllocAffinityResponse
#   - rsp.err_code: 错误码
#   - rsp.workflow_id: 工作流 ID

# 获取 SC 元数据
sc = self.sys_admin.get_sc_meta(scg_id, sc_name)
# 返回: SubCluster 对象或 None
```

### 2.5 工作流管理

```python
# 检查工作流是否完成
res = self.sys_admin.check_workflow_finished(
    workflow_id,
    status=4,           # 期望状态：4=完成，6=回滚，3=错误
    max_retry_time=3600,# 最大等待时间（秒）
    need_assert=True    # 是否断言
)
# 返回: bool

# 获取工作流详情
rsp = self.sys_admin.get_workflow(workflow_id)
# 返回: GetWorkflowResponse
#   - rsp.info: WorkflowInfo
#   - rsp.info.status: 工作流状态
#   - rsp.info.result.result: JSON 格式的结果

# 运行工作流步骤（用于 debug 模式）
rsp = self.sys_admin.run_step_as_expect(
    workflow_id=workflow_id,
    expect=STEP_EXPECT_NEXT    # NEXT/REPEAT/FAIL
)
```

### 2.6 扩展字段管理

```python
# 获取账户扩展
rsp = self.sys_admin.get_account_extension(account_id, "")
# 返回: GetAccountExtensionResponse
#   - rsp.err_code: 错误码，-201064 表示不存在
#   - rsp.extension: ExtensionMeta

# 创建账户扩展
rsp = self.sys_admin.create_account_extension(account_id, {"key": "value"})

# 更新账户扩展
rsp = self.sys_admin.update_account_extension(account_id, {"key": "new_value"})

# 删除账户扩展
rsp = self.sys_admin.delete_account_extension(account_id)
```

## 三、UserAdminDao API

**文件位置**：`test/dao/user_admin_dao.py`

### 3.1 账户操作

```python
# 创建数据库账户
rsp = self.user_admin.create_database_account(
    app_id,
    name=account_name,
    read_ru=0,
    write_ru=10000,
    cache_cap=50000,
    storage_cap=1000000,
    cluster_name=cluster_name,
    api_type=APIType.API_TYPE_TSSD,
    alloc_affinity=""
)
# 返回: CreateDatabaseAccountResponse
#   - rsp.err_code: 错误码
#   - rsp.workflow_id: 工作流 ID

# 删除数据库账户
rsp = self.user_admin.delete_database_account(account_id=account_id)

# 获取账户 ID
account_id = self.user_admin.get_database_account_id(app_id, account_name)
```

### 3.2 数据库操作

```python
# 创建数据库
rsp = self.user_admin.create_database(
    account_id,
    database_name,
    partition_count=18,
    replica_count=3,
    read_ru=0,
    write_ru=0,
    cache_cap=0,
    storage_cap=100000
)

# 删除数据库
rsp = self.user_admin.delete_database(database_id)

# 获取数据库 ID
db_id = self.user_admin.get_database_id(
    database_account_id=account_id,
    database_name=db_name
)
```

### 3.3 Collection 操作

```python
# 创建 Collection
rsp = self.user_admin.create_collection(
    database_id,
    collection_name,
    storage_cap=20,
    collection_alias=alias
)

# 删除 Collection
rsp = self.user_admin.delete_collection(collection_id)

# 获取 Collection ID
coll_id = self.user_admin.get_collection_id(database_id, collection_name)
```

## 四、ControlPlane 统一入口

**文件位置**：`test/dao/controlplane.py`

```python
class ControlPlane:
    def __init__(self, cluster):
        self.sys_admin = SystemAdminDao(cluster)
        self.user_admin = UserAdminDao(cluster)
        self.monitor = MonitorDao(cluster)
        self.global_dao = GlobalClient()
    
    def delete_resources_by_account_id(self, account_id, need_assert=True):
        """删除账户及其所有资源"""
    
    def get_resources_by_account(self, account_id):
        """获取账户下的所有资源"""
        return databases, collections
```

## 五、YottadbLib 工具库

**文件位置**：`test/yottadblib/yottadb_lib.py`

### 5.1 资源创建

```python
# 创建数据库资源
dba_id, db_id, coll_id = self.yottadb_lib.create_database_resource(
    app_id=app_id,
    dba_name=account_name,
    db_name=database_name,
    coll_name=collection_name,
    storage_cap=storage,
    cluster_name=cluster_name,
    consistency=consistency,
    api_type=api_type,
    partition_count=partition_count,
    replica_cnt=replica_cnt,
    alloc_affinity=alloc_affinity
)
```

### 5.2 Pod 操作

```python
# 获取 Pod DNS
pod_dns = self.yottadb_lib.get_pod_dns_by_pod_name(pod_name)

# 获取 Gateway 节点
gateway_info = self.yottadb_lib.get_gateway_nodes_by_dba_name(
    app_id=app_id,
    dba_name=account_name
)
# 返回: List[Tuple[node_ip, port]]
```

### 5.3 Debug 模式

```python
# 更新集群 debug 模式
self.yottadb_lib.update_cluster_engine_debug_mode(
    cls_id=cluster_id,
    debug_mode=True  # True=开启，False=关闭
)
```

## 六、Protocol Buffer 消息

### 6.1 常用消息结构

**BaseInfo（基础信息）**：
```python
base_info._pid      # 父 ID
base_info._rid      # 资源 ID
base_info.name      # 名称
base_info._version  # 版本号
base_info.status    # ResourceStatus 枚举
```

### 6.2 常用枚举

```python
from trpc_client.stub.common_pb2 import (
    ResourceStatus,      # 资源状态
    RaftRole,            # Raft 角色
    APIType,             # API 类型
    ConsistencyPolicy,   # 一致性策略
)

# ResourceStatus
ResourceStatus.RESOURCE_STATUS_DEFAULT     # 0
ResourceStatus.RESOURCE_STATUS_CREATING    # 1
ResourceStatus.RESOURCE_STATUS_RUNNING     # 2
ResourceStatus.RESOURCE_STATUS_DELETING    # 3
ResourceStatus.RESOURCE_STATUS_DELETED     # 4

# RaftRole
RaftRole.RAFT_ROLE_LEADER
RaftRole.RAFT_ROLE_FOLLOWER
RaftRole.RAFT_ROLE_WITNESS

# APIType
APIType.API_TYPE_TSSD
APIType.API_TYPE_REDIS_V2
APIType.API_TYPE_BDB_ROCKSDB
APIType.API_TYPE_BDB_FASTERKV
APIType.API_TYPE_INFLUXDB
```

### 6.3 WorkflowStatus

```python
# 工作流状态
WorkflowNotStarted = 0      # 未启动
WorkflowRunning = 1         # 运行中
WorkflowPaused = 2          # 已暂停
WorkflowPermanentError = 3  # 永久错误
WorkflowFinished = 4        # 已完成
WorkflowRollingBack = 5     # 回滚中
WorkflowRolledBack = 6      # 已回滚
```

## 七、错误码

### 7.1 常用错误码

| 错误码 | 说明 |
|--------|------|
| 0 | OK |
| -201064 | 资源不存在 |
| 其他 | 参考 conf/error_codes.yml |
