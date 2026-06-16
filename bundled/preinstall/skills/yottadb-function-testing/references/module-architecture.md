# 模块架构参考

本文档详细描述 YottaDB 测试项目的各模块架构、基类选择和特定 API。

## 一、模块概览

### 1.1 模块与基类映射

| 模块 | 基类 | 文件路径 | 使用场景 |
|------|------|----------|----------|
| TSDB | `TestTsdbBase` | `case/tsdb/test_tsdb_base.py` | 时序数据库功能测试 |
| Data Plane | `TestClusterCaseBase` | `case/data_plane/test_cluster_case_base.py` | SSDKV 数据面测试 |
| Control Plane | `TestControlPlaneBase` | `case/control_plane/test_control_plane_base.py` | SSDKV 控制面测试 |
| MEMDB | `TestMemdbBase` | `case/memdb/test_memdb_base.py` | 内存数据库测试 |
| FasterKV | `TestClusterCaseBase` | `case/data_plane/test_cluster_case_base.py` | FasterKV 存储测试 |
| BDB | `TestClusterCaseBase` | `case/data_plane/test_cluster_case_base.py` | BDB 数据库测试 |
| Marvel | `TestMarvelBase` | `case/marvel/test_marvel_base.py` | Marvel 管道测试 |

### 1.2 继承层次详解

```
TestCase (yottadblib/case.py)
│   基础测试用例类，提供最基本的测试框架
│
└── TestSystemCase (case/system_case.py)
    │   系统级测试基类，提供：
    │   - cp_cluster / dp_cluster 集群配置
    │   - control_plane 控制面入口
    │   - clean_resources() 资源清理
    │   - set_resource_name() 资源命名
    │
    ├── TestClusterCaseBase (case/data_plane/test_cluster_case_base.py)
    │   │   数据面测试基类，提供：
    │   │   - 资源创建 create_resource_and_init_info()
    │   │   - 压测工具 start_presscall_read_and_write()
    │   │   - Gateway 操作
    │   │   - Partition/Replica 操作
    │   │
    │   └── TestTsdbBase (case/tsdb/test_tsdb_base.py)
    │           TSDB 专用基类，提供：
    │           - create_tsdb_resource_and_init_info()
    │           - occupy_sc() 占用 SubCluster
    │           - clean_account_and_resources()
    │           - TSDB 特定的 API 封装
    │
    ├── TestControlPlaneBase (case/control_plane/test_control_plane_base.py)
    │       控制面测试基类（轻量级）
    │
    └── TestMemdbBase (case/memdb/test_memdb_base.py)
            内存数据库测试基类
```

## 二、TSDB 模块详解

### 2.1 目录结构

```
case/tsdb/
├── test_tsdb_base.py          # TSDB 测试基类
├── test_access_pool_base.py   # AccessPool 测试基类
├── test_scheduler_base.py     # 调度器测试基类
├── access_pool/               # AccessPool 测试
├── account_crud/              # 账户 CRUD 测试
├── backup/                    # 备份测试
├── collection_crud/           # Collection CRUD
├── database_crud/             # Database CRUD
├── infra_crud/                # 基础设施 CRUD
├── kafka_ingest/              # Kafka 摄入测试
├── migrate/                   # 迁移测试
├── monitoring/                # 监控测试
├── placement/                 # 调度放置测试
├── scheduler/                 # 调度器测试
├── tiering/                   # 分层存储测试
├── upgrade/                   # 升级测试
└── wal/                       # WAL 测试
```

### 2.2 TestTsdbBase 关键方法

```python
class TestTsdbBase(TestClusterCaseBase):
    """
    TSDB 测试基类，提供 TSDB 场景的通用方法
    """
    
    # ========== 资源创建 ==========
    def create_tsdb_resource_and_init_info(
        self, 
        dba_name,           # 账户名称
        db_name,            # 数据库名称
        cluster_name,       # 集群名称
        alloc_affinity="",  # 分配亲和性
        partition_num_per_coll=1,  # 每个 collection 的分区数
        replica_count=3     # 副本数
    ) -> str:
        """创建 TSDB 资源并初始化，返回 account_id"""
    
    # ========== SC 操作 ==========
    def occupy_sc(self, sc_cnt=1, alloc_affinity=""):
        """为当前账户占用指定数量的 SubCluster"""
    
    def get_sc_ids_by_alloc_affinity(self, alloc_affinity) -> List[str]:
        """根据亲和性获取 SC ID 列表"""
    
    def release_sc_alloc_affinity(self):
        """释放所有占用的 SC 亲和性"""
    
    # ========== 资源清理 ==========
    def clean_account_and_resources(self):
        """清理当前账户及其所有资源"""
    
    # ========== 工作流操作 ==========
    def wait_workflow_step_finished(self, workflow_id, expected_step):
        """等待工作流执行到指定步骤"""
    
    # ========== 基类属性 ==========
    # self.dp_cluster - 数据面集群配置
    # self.cp_cluster - 控制面集群配置
    # self.cluster_id - 集群 ID
    # self.control_plane - ControlPlane 实例
    # self.sys_admin - SystemAdminDao 实例
    # self.user_admin - UserAdminDao 实例
    # self.yottadb_lib - YottadbLib 实例
    # self.account_id - 当前账户 ID
    # self.db_id - 当前数据库 ID
    # self.coll_id - 当前 Collection ID
    # self.occupy_sc_list - 占用的 SC 列表
```

## 三、Control Plane 模块详解

### 3.1 目录结构

```
case/control_plane/
├── test_control_plane_base.py  # 控制面测试基类
├── test_infra_base.py          # 基础设施测试基类
├── test_migrate_base.py        # 迁移测试基类
├── test_scheduler_base.py      # 调度器测试基类
├── account_alias/              # 账户别名测试
├── backup/                     # 备份测试
├── cache_crud/                 # Cache CRUD 测试
├── collection_alias/           # Collection 别名
├── collection_crud/            # Collection CRUD
├── database_account_crud/      # 账户 CRUD
├── database_crud/              # Database CRUD
├── infrastructure_crud/        # 基础设施 CRUD
├── migration/                  # 迁移测试
├── multi_region/               # 多地域测试
├── scheduler/                  # 调度器测试
├── split/                      # 分裂测试
├── upgrade/                    # 升级测试
└── workflow/                   # 工作流测试
```

### 3.2 控制面专用基类

| 基类 | 适用场景 |
|------|----------|
| `TestControlPlaneBase` | 通用控制面测试 |
| `TestInfraBase` | 基础设施（Node、SCG、SC）测试 |
| `TestMigrateBase` | 迁移测试 |
| `TestSchedulerBase` | 调度器测试 |
| `TestBackupBase` | 备份测试 |
| `TestMultiRegionBase` | 多地域测试 |

## 四、Data Plane 模块详解

### 4.1 目录结构

```
case/data_plane/
├── test_cluster_case_base.py      # 数据面测试基类
├── test_data_access_base.py       # 数据访问测试基类
├── test_data_admin_base.py        # 数据管理测试基类
├── test_tssd_base.py              # TSSD 测试基类
├── Cache/                         # Cache 测试
├── data_admin/                    # 数据管理测试
├── database/                      # 数据库测试
├── gateway/                       # Gateway 测试
└── ru/                            # RU 测试
```

### 4.2 TestClusterCaseBase 关键方法

```python
class TestClusterCaseBase(TestSystemCase):
    """数据面测试基类"""
    
    def create_resource_and_init_info(
        self, 
        dba_name, db_name, coll_name, cluster_name,
        storage=50,
        consistency=ConsistencyPolicy.CONSISTENCY_POLICY_STRONG,
        api_type=APIType.API_TYPE_TSSD,
        partition_count=0,
        replica_cnt=3,
        ...
    ) -> str:
        """创建资源并初始化信息，返回 partition_id"""
    
    def start_presscall_read_and_write(
        self, client_path, gw_host, gw_port, 
        tid, cid, request_type,
        thread=10, key_size=8, value_size=1024,
        key_range=10000, run_mins=5, qps_limit=10000
    ):
        """启动压测读写"""
        return queue, process
    
    def check_presscall_result(self, queue, process, require_suc_rate, timeout=1200):
        """检查压测结果"""
    
    def switch_primary_by_collection_id(self, collection_id, times=1):
        """按 collection_id 切换主副本"""
```

## 五、模块选择决策树

```
收到测试用例描述
    │
    ├── 涉及时序数据/InfluxDB 协议？
    │   └── Yes → TSDB 模块，使用 TestTsdbBase
    │
    ├── 涉及 Redis 协议/内存存储？
    │   └── Yes → MEMDB 模块，使用 TestMemdbBase
    │
    ├── 涉及 FasterKV 存储引擎？
    │   └── Yes → FasterKV 模块，使用 TestClusterCaseBase
    │
    ├── 涉及 BDB/RocksDB？
    │   └── Yes → BDB 模块，使用 TestClusterCaseBase
    │
    ├── 涉及数据写入/读取/Gateway？
    │   └── Yes → Data Plane 模块，使用 TestClusterCaseBase
    │
    ├── 涉及资源管理/调度/基础设施？
    │   └── Yes → Control Plane 模块，使用对应基类
    │
    └── 涉及流数据/管道？
        └── Yes → Marvel 模块，使用 TestMarvelBase
```

## 六、跨模块通用组件

### 6.1 DAO 层

```python
# 控制面 DAO
from dao.controlplane import ControlPlane
from dao.sys_admin_dao import SystemAdminDao
from dao.user_admin_dao import UserAdminDao

# 数据面 DAO
from dao.data_admin_dao import DataAdminDao
from dao.data_access_dao import DataAccessDao

# 基础设施 DAO
from dao.kubernets_dao import K8sDao
from dao.chaos_dao import Blade
```

### 6.2 Protocol Buffer

```python
from trpc_client.stub.common_pb2 import (
    ResourceStatus,
    RaftRole,
    APIType,
    ConsistencyPolicy,
)
```

### 6.3 工具库

```python
from yottadblib.yottadb_lib import YottadbLib
from yottadblib.resource_util import gen_replica_id_by_index
```
