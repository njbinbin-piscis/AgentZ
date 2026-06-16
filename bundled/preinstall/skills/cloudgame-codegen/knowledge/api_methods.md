# API 方法速查（防幻觉清单）

> 🚫 **防幻觉铁律**：本文档是生成代码时**唯一允许引用的 API 清单**。**每一个被调用的方法都必须能在本文档中找到对应条目**。不在此清单中的方法一律视为不存在，禁止调用。
>
> **遇到找不到对应 API 的情况时**：必须从本文档已列出的方法中选择功能最接近的来实现，不得编造不存在的方法。
>
> **⚠️ 查阅规则**：生成代码前，**必须先查阅本文档**，确认方法名、必填参数名称和类型、返回值字段，不得凭记忆或猜测使用。

## ⚠️ 返回值字段防幻觉铁律（适用于所有 API）

> **核心原则**：调用任意关键字或 API 时，**只能使用本文档中明确列出的返回值字段**。
> **严禁凭直觉或经验猜测字段名**（如 `stdout`、`output`、`result`、`data`、`status` 等），未在文档中列出的字段一律视为不存在。

### 通用规则

```python
# ✅ 正确：只使用文档中明确列出的字段
ret, result = some_api_call(...)
value = result.get("doc_field_name", "")   # 字段名来自本文档

# ❌ 错误：使用未在文档中列出的字段（即使看起来合理）
value = result.get("stdout", "")    # 禁止！未在文档中列出
value = result.get("output", "")    # 禁止！未在文档中列出
value = result.get("data", "")      # 禁止！未在文档中列出（除非文档明确列出）
value = result["result"]            # 禁止！未在文档中列出
```

### 遇到文档未列出返回值字段时

1. **不得编造字段名**，必须查阅本文档或实际源码
2. 如本文档未列出该 API 的返回值结构，**只能使用最保守的方式**：直接使用返回值整体（如 `ret` 布尔值），不拆解字段
3. 如需拆解字段，必须先在本文档中补充该 API 的返回值结构说明

---

## API 类速查

| 模块 | 文件 | 类 | 默认端口 |
|------|------|---|---------|
| proxy（统一入口） | `libs/api/proxy_api.py` | `ProxyAPI(ip)` | 24756 |
| resource_svr | `libs/api/resource_svr_api.py` | `Resource_Svr_Request(ip)` | 8080 |
| cgmanager | `libs/api/cgmanager_api.py` | `CG_Manager_Request(ip)` | 32767 |
| master | `libs/api/master_api.py` | `Master_Request(ip)` | 10086 |
| oss | `libs/api/oss_api.py` | `Game_Oss_Request(ip)` | 80 |
| cdn | `libs/api/cdn_api.py` | `Game_Stream_Request(cdn_ip, cdn_host)` | 80 |
| docker_midserver | `libs/api/docker_midserver_api.py` | `Docker_Midserver_Request(ip, test_ip)` | 2375 |

> **ProxyAPI 说明**：`ProxyAPI` 是统一入口，内部聚合了 `game_dispatch_api.py`、`op_image_api.py`、`op_instance_api.py`、`op_resource_api.py`、`op_wx_video.py` 的所有方法，通过 proxy 服务转发。**步骤描述"通过 proxy 调用"时，必须使用 ProxyAPI，不得直接使用底层类。**

---

## ProxyAPI 方法（`libs/api/proxy_api.py`）

### 游戏调度（来自 `game_dispatch_api.py`）

#### `alloc_instance_v2(**kwargs)` — 占用游戏实例

- **URL**：`POST /cloudrender/cloudgame/alloc_instance_v2`
- **必填参数**：
  - `region` (str)：大区
  - `appid` (str)：游戏标识
  - `session_id` (str)：会话ID
  - `user_id` (str)：用户ID
  - `instance_id` (str)：实例ID
  - `business_id` (str)：业务ID
- **可选参数**：
  - `client_ip` (str)：客户端IP，默认 `"11.141.162.236"`
  - `resolution` (int)：分辨率，默认 `1`
- **返回值**：`(code, resp)`，`resp` 结构未在源码中明确定义，只使用 `code` 判断成功

#### `stop_game(**kwargs)` — 停止游戏

- **URL**：`POST /cloudrender/cloudgame/online/stop_game`
- **必填参数**：
  - `session_id` (str)：会话ID
- **返回值**：`(code, resp)`

#### `resources_v4(**kwargs)` — 获取资源配置信息

- **URL**：`GET /cloudrender/cloudgame/resources_v4?query=encodeParam`
- **必填参数**：
  - `business_id` (int)：固定为 `1000`
  - `application_type` (int)：固定为 `0`
  - `version` (str)：固定为 `"latest"`
- **可选参数**：
  - `resolution` (int)：分辨率（1/2/3/4）
  - `hardware` (str)：机型（`snapdragon865`/`tianji`/`windows_pc`）
  - `appids` (list)：应用ID列表
- **返回值**：`(code, resp)`

#### `sync_msg(**kwargs)` — 验证业务控制信息

- **URL**：`POST /cloudrender/cloudgame/online/sync_msg`
- **必填参数**：
  - `id` (str)：任务唯一id
  - `session_id` (str)：会话ID
  - `msg_body` (dict)：消息体，必须包含 `conn_id` 字段
- **返回值**：`(code, resp)`

---

### 镜像操作（来自 `op_image_api.py`）

#### `get_arm_image_docker(**kwargs)` — 获取构建镜像容器（ARM）

- **URL**：`POST /api/v1/arm/service/provider?business_id=xxx`
- **必填参数**：`business_id` (str)、`provider` (str)
- **请求体**：`{"provider": ...}`
- **返回值**：`(code, resp)`

#### `get_x86_image_docker(**kwargs)` — 获取构建镜像容器（x86）

- **URL**：`POST /api/v1/x86/service/provider?business_id=xxx`
- **必填参数**：`business_id` (str)、`provider` (str)
- **请求体**：`{"provider": ...}`
- **返回值**：`(code, resp)`

#### `sync_adb_shell(**kwargs)` — adb shell 同步接口

- **URL**：`POST /api/v1/instance/shell/command?business_id=xxx`
- **必填参数**：`business_id` (str)、`provider` (str)、`instance_id` (str)、`cmd` (str)
- **请求体**：`{"provider", "instance_id", "cmd"}`
- **返回值**：`(code, resp)`

#### `save_arm_image(**kwargs)` — 保存 ARM 镜像

- **URL**：`POST /api/v1/upload/instance/game/image?business_id=xxx`
- **必填参数**：`business_id` (str)、`provider` (str)、`instance_id` (str)
- **请求体**：`{"provider", "instance_id"}`
- **返回值**：`(code, resp)`

#### `save_x86_image(**kwargs)` — 保存 x86 镜像

- **URL**：`POST /api/v1/upload/x86/game/image?business_id=xxx`
- **必填参数**：`business_id` (str)、`provider` (str)、`instance_id` (str)
- **请求体**：`{"provider", "instance_id"}`
- **返回值**：`(code, resp)`

#### `query_image_status(**kwargs)` — 查询镜像状态

- **URL**：`GET /api/v1/instance/game/images?provider=xxx&game_image_id=xxx&business_id=xxx`
- **必填参数**：`game_image_id` (str)、`provider` (str)、`business_id` (str)
- **返回值**：`(code, resp)`

#### `upload_apk(**kwargs)` — 安装 APK

- **URL**：`POST /api/v1/upload/apk/to/instance?business_id=xxx`
- **必填参数**：`business_id` (str)、`provider` (str)、`instance_id` (str)、`file_url` (str)
- **请求体**：`{"provider", "instance_id", "file_url"}`
- **返回值**：`(code, resp)`

#### `download_file(**kwargs)` — 下载文件

- **URL**：`POST /api/v1/upload/apk/to/instance?business_id=xxx`
- **必填参数**：`business_id` (str)、`provider` (str)、`instance_id` (str)、`file_url` (str)、`upload_path` (str)
- **请求体**：`{"provider", "instance_id", "file_url", "upload_path"}`
- **返回值**：`(code, resp)`

#### `async_query_task(**kwargs)` — 异步任务查询

- **URL**：`GET /api/v1/task/detail?business_id=xxx`
- **必填参数**：`business_id` (str)、`provider` (str)、`task_id` (str)
- **请求体**：`{"provider", "task_id"}`
- **返回值**：`(code, resp)`

#### `async_adb_shell(**kwargs)` — adb shell 异步接口

- **URL**：`POST /api/v1/task/instance/shell/command?business_id=xxx`
- **必填参数**：`business_id` (str)、`provider` (str)、`instance_id` (str)、`cmd` (str)
- **可选参数**：`timeout` (int)，默认 `30`
- **请求体**：`{"provider", "instance_id", "cmd", "timeout"}`
- **返回值**：`(code, resp)`

#### `init_game_image(**kwargs)` — 实例初始化游戏镜像

- **URL**：`POST /api/v1/init/instance/game/image?business_id=xxx`
- **必填参数**：`business_id` (str)、`provider` (str)、`instance_id` (str)、`from_game_image_id` (str)
- **请求体**：`{"provider", "instance_id", "from_game_image_id"}`
- **返回值**：`(code, resp)`

#### `get_instance_init_status(**kwargs)` — 获取实例初始化状态

- **URL**：`GET /api/v1/instance/init/status?business_id=xxx&instance_id=xxx&provider=xxx`
- **必填参数**：`business_id` (str)、`provider` (str)、`instance_id` (str)
- **返回值**：`(code, resp)`

#### `reset_tmp_instance(**kwargs)` — 重置模板实例

- **URL**：`POST /api/v1/del/template/instance?business_id=xxx`
- **必填参数**：`business_id` (str)、`provider` (str)、`instance_id` (str)
- **请求体**：`{"provider", "instance_id"}`
- **返回值**：`(code, resp)`

#### `upload_image_to_cloud(**kwargs)` — 上传游戏镜像到云端

- **URL**：`POST /api/v1/upload/game/image/cloud?business_id=xxx`
- **必填参数**：`business_id` (str)、`provider` (str)、`instance_id` (str)
- **请求体**：`{"provider", "instance_id"}`
- **返回值**：`(code, resp)`

#### `load_cloud_image(**kwargs)` — 导入云端镜像到厂商

- **URL**：`POST /api/v1/load/game/image/cloud?business_id=xxx`
- **必填参数**：`business_id` (str)、`provider` (str)、`cloud_path` (str)
- **请求体**：`{"provider", "cloud_path"}`
- **返回值**：`(code, resp)`

---

### 实例操作（来自 `op_instance_api.py`）

#### `get_instance_info_by_proxy(**kwargs)` — 获取实例信息

- **URL**：`GET /api/v1/describe/arm/service/instances/by/instance_id?business_id=xxx&provider=xxx&instance_id=xxx`
- **必填参数**：`business_id` (str)、`provider` (str)、`instance_id` (str)
- **返回值**：`(code, resp)`，`resp` 结构：
  ```json
  {
    "ret": {"code": 0, "msg": "success"},
    "body": {
      "instance_info": {
        "instance_id": "xxxx", "provider": "xxx", "business_id": "xxxx",
        "status": "running", "region_id": "dongguan-node-8",
        "spec": "c2", "resolution": "1920*1080",
        "image_id": "5302d883ea7215b6dc7e913bb0e0bebc"
      }
    }
  }
  ```
  - ✅ 可用字段：`resp["body"]["instance_info"]["instance_id"]`、`["status"]`、`["region_id"]`、`["spec"]`、`["resolution"]`、`["image_id"]`

#### `mount_game_image(**kwargs)` — 游戏镜像挂载（proxy → 实例层）

- **URL**：`POST /api/v1/arm/service/instance/mount/game/image?business_id=xxx`
- **必填参数**：
  - `business_id` (str)
  - `game_image_id` (str)
  - `instance_ids` (list)：**注意是列表，不是单个字符串**
  - `provider` (str)
- **请求体**：`{"game_image_id", "instance_ids", "provider"}`
- **返回值**：`(code, resp)`，`resp` 结构：
  ```json
  {"ret": {"code": 0, "msg": "success"}, "body": {"task_id": "190"}}
  ```
  - ✅ 可用字段：`resp["body"]["task_id"]`

#### `get_region_ids(**kwargs)` — 厂商节点查询

- **URL**：`GET /api/v1/arm/service/region_ids?business_id=xxx&provider=xxx`
- **必填参数**：`business_id` (str)、`provider` (str)
- **返回值**：`(code, resp)`，`resp` 结构：
  ```json
  {"ret": {"code": 0, "msg": "success"}, "body": {"region_ids": ["dongguan-node-1", ...]}}
  ```
  - ✅ 可用字段：`resp["body"]["region_ids"]`

#### `get_instances_by_region(**kwargs)` — 查询实例信息（节点）

- **URL**：`GET /api/v1/describe/arm/service/instances/by/region_id?...`
- **必填参数**：`business_id` (str)、`provider` (str)、`region_id` (str)
- **可选参数**：`page_number` (int，默认1)、`page_size` (int，默认100)
- **返回值**：`(code, resp)`，`resp` 结构：
  ```json
  {
    "ret": {"code": 0, "msg": "success"},
    "body": {
      "servers": [{"server_id": "...", "region_id": "...", "status": "running",
                   "image_id": "...", "instances": [{"instance_id": "...", "resolution": "...",
                   "status": "running", "ip_address": "...", "game_image_id": "...", "spec": "c1"}]}],
      "count": 100
    }
  }
  ```
  - ✅ 可用字段：`resp["body"]["servers"]`、`resp["body"]["count"]`、`servers[i]["instances"]`、`instances[j]["instance_id"]`、`["status"]`、`["ip_address"]`、`["game_image_id"]`

#### `get_x86_instances_by_region(**kwargs)` — 查询 x86 实例信息（节点）

- **URL**：`GET /api/v1/describe/x86/service/instances/by/region_id?...`
- **必填参数**：`business_id` (str)、`provider` (str)、`region_id` (str)
- **可选参数**：`page_number` (int，默认1)、`page_size` (int，默认100)
- **返回值**：同 `get_instances_by_region`

#### `reset_instance(**kwargs)` — 重置实例

- **URL**：`POST /api/v1/reset/arm/service/instance?business_id=xxx`
- **必填参数**：`business_id` (str)、`instance_id` (str)、`provider` (str)
- **请求体**：`{"instance_id", "provider"}`
- **返回值**：`(code, resp)`，`resp` 结构：`{"ret": {"code": 0, "msg": "success"}, "body": {}}`

#### `reboot_instance(**kwargs)` — 重启实例

- **URL**：`POST /api/v1/reboot/arm/service/instance?business_id=xxx`
- **必填参数**：`business_id` (str)、`instance_id` (str)、`provider` (str)
- **请求体**：`{"instance_id", "provider"}`
- **返回值**：`(code, resp)`，`resp` 结构：`{"ret": {"code": 0, "msg": "success"}, "body": {}}`

#### `set_instance_properties(**kwargs)` — 实例设置属性

- **URL**：`POST /api/v1/put/arm/service/instance/property?business_id=xxx`
- **必填参数**：`business_id` (str)、`instance_id` (str)、`provider` (str)、`properties` (dict，key/value 均为 str，不能为空)
- **请求体**：`{"instance_id", "provider", "properties"}`
- **返回值**：`(code, resp)`，`resp` 结构：`{"ret": {"code": 0, "msg": "success"}, "body": {}}`

#### `get_instance_properties(**kwargs)` — 实例获取属性

- **URL**：`POST /api/v1/get/arm/service/instance/property?business_id=xxx`
- **必填参数**：`business_id` (str)、`instance_id` (str)、`provider` (str)、`keys` (list[str]，非空)
- **请求体**：`{"instance_id", "provider", "keys"}`
- **返回值**：`(code, resp)`，`resp` 结构：`{"ret": {"code": 0, "msg": "success"}, "body": {"ro.instance.id": "..."}}`
  - ✅ 可用字段：`resp["body"]` 中以 key 名称为键的属性值

#### `reboot_arm_service(**kwargs)` — 重启板卡

- **URL**：`POST /api/v1/reboot/arm/service?business_id=xxx`
- **必填参数**：`business_id` (str)、`server_id` (str)、`provider` (str)
- **请求体**：`{"server_id", "provider"}`
- **返回值**：`(code, resp)`，`resp` 结构：`{"ret": {"code": 0, "msg": "success"}, "body": {}}`

#### `upgrade_arm_service_image(**kwargs)` — 升级板卡镜像

- **URL**：`POST /api/v1/upgrade/arm/service/instance?business_id=xxx`
- **必填参数**：`business_id` (str)、`provider` (str)、`server_ids` (list[str]，非空)、`image_id` (str)
- **请求体**：`{"provider", "server_ids", "image_id"}`
- **返回值**：`(code, resp)`，`resp` 结构：`{"ret": {"code": 0, "msg": "success"}, "body": {}}`

#### `get_instance_distribution_status(**kwargs)` — 查询实例分布状态

- **URL**：`POST /api/v1/stat/arm/service/instances/distribution_status?business_id=xxx`
- **必填参数**：`business_id` (str)
- **可选参数**：`region_ids` (list[str])，默认 `[]`
- **请求体**：`{"region_ids": [...]}`
- **返回值**：`(code, resp)`，`resp` 结构：
  ```json
  {"ret": {"code": 0, "msg": "success"}, "body": [{"num": 8, "region_id": "...", "spec": "c2", "status": "已申请"}]}
  ```
  - ✅ 可用字段：`resp["body"]` 列表，每项含 `num`、`region_id`、`spec`、`status`

#### `upgrade_instance_system_image(**kwargs)` — 实例镜像升级

- **URL**：`POST /api/v1/upgrade/arm/service/instance/system/image?business_id=xxx`
- **必填参数**：`business_id` (str)、`instance_id` (str)、`image_id` (str)、`provider` (str)
- **请求体**：`{"instance_id", "image_id", "provider"}`
- **返回值**：`(code, resp)`，`resp` 结构：`{"ret": {"code": 0, "msg": "success"}, "body": {}}`

#### `reboot_x86_cvm(**kwargs)` — x86 CVM 重启

- **URL**：`POST /api/v1/x86/service/server/reboot?business_id=xxx`
- **必填参数**：`business_id` (str)、`server_id` (str)、`provider` (str)
- **请求体**：`{"server_id", "provider"}`
- **返回值**：`(code, resp)`，`resp` 结构：`{"ret": {"code": 0, "msg": "success"}, "body": {}}`

#### `create_x86_instance_config(**kwargs)` — x86 实例配置生成

- **URL**：`POST /api/v1/x86/service/instance/create?business_id=xxx`
- **必填参数**：`server_id` (str)、`game_image_id` (str)、`num` (int)、`provider` (str，固定 `"winteg"`)、`business_id` (str，固定 `"3000"`)
- **请求体**：`{"server_id", "game_image_id", "num", "provider", "business_id"}`
- **返回值**：`(code, resp)`，`resp` 结构：
  ```json
  {"ret": 0, "msg": "", "data": {"card_type": "...", "region_id": "...", "instances": [{"instance_id": "...", "channel_id": 0, "data_vol_version": "...", "app_id": "...", "host_ip": "..."}]}}
  ```
  - ✅ 可用字段：`resp["data"]["instances"]`，每项含 `instance_id`、`channel_id`、`host_ip`

#### `get_x86_instance_config(**kwargs)` — x86 实例配置获取

- **URL**：`POST /api/v1/x86/service/instance/info`
- **必填参数**：`server_id` (str)、`provider` (str，固定 `"winteg"`)、`business_id` (str，固定 `"3000"`)
- **请求体**：`{"server_id", "provider", "business_id"}`
- **返回值**：同 `create_x86_instance_config`

#### `delete_x86_instance_config(**kwargs)` — x86 沙盒配置删除

- **URL**：`POST /api/v1/x86/service/instance/delete?business_id=xxx`
- **必填参数**：`server_id` (str)、`provider` (str，固定 `"winteg"`)、`business_id` (str，固定 `"3000"`)
- **请求体**：`{"server_id", "provider", "business_id"}`
- **返回值**：`(code, resp)`，`resp` 结构：`{"ret": 0, "msg": ""}`

#### `get_x86_region_ids(**kwargs)` — x86 节点查询

- **URL**：`GET /api/v1/x86/service/region_ids?business_id=xxx&provider=xxx`
- **必填参数**：`business_id` (str)、`provider` (str)
- **返回值**：`(code, resp)`，`resp` 结构：`{"ret": {"code": 0, "msg": "success"}, "body": {"region_ids": [...]}}`

#### `describe_x86_service_instances(**kwargs)` — x86 实例信息获取

- **URL**：`GET /api/v1/describe/x86/service/instances/by/instance_id?business_id=xxx&instance_id=xxx&provider=xxx`
- **必填参数**：`business_id` (str)、`instance_id` (str)、`provider` (str)
- **返回值**：`(code, resp)`，`resp` 结构：
  ```json
  {"ret": {"code": 0, "msg": "success"}, "body": {"servers": [{"server_id": "...", "instances": [{"instance_id": "...", "status": "running", "ip_address": "...", "game_image_id": "...", "adb_port": 0}]}], "count": 1}}
  ```
  - ✅ 可用字段：`resp["body"]["servers"]`，`servers[i]["instances"][j]["instance_id"]`、`["status"]`、`["ip_address"]`、`["game_image_id"]`、`["adb_port"]`

#### `mount_x86_game_image(**kwargs)` — x86 游戏镜像挂载

- **URL**：`POST /api/v1/x86/service/instance/mount/game/image?business_id=xxx`
- **必填参数**：`business_id` (str)、`game_image_id` (str)、`instance_ids` (list)、`provider` (str)
- **请求体**：`{"game_image_id", "instance_ids", "provider"}`
- **返回值**：`(code, resp)`，`resp` 结构：`{"ret": {"code": 0, "msg": "success"}, "body": {}}`

#### `get_x86_server_info(**kwargs)` — 获取 x86 远程桌面信息

- **URL**：`POST /api/v1/x86/service/server/info`
- **必填参数**：`server_id` (str)、`provider` (str，固定 `"winteg"`)、`business_id` (str，固定 `"3000"`)
- **请求体**：`{"server_id", "provider", "business_id"}`
- **返回值**：`(code, resp)`，`resp` 结构：
  ```json
  {"code": 0, "msg": "success", "data": {"card_type": "3060", "region_id": "...", "instances": [{"app_id": "remotedesktop", "channel_id": 231401, "host_ip": "...", "instance_id": "...", "data_vol_version": "0.0.1"}]}}
  ```
  - ✅ 可用字段：`resp["data"]["instances"]`，每项含 `instance_id`、`channel_id`、`host_ip`

---

### 资源操作（来自 `op_resource_api.py`）

#### `instance_level_mounting(**kwargs)` — 实例维度挂载游戏镜像

- **URL**：`POST /api/proxy/resource/mount/game/image?business_id=xxx`
- **必填参数**：
  - `business_id` (str)：业务ID
  - `instance_ids` (list)：实例ID列表
  - `game_image_id` (str)：游戏镜像ID
- **请求体**：`{"instance_ids", "game_image_id"}`（business_id 在 URL query 中）
- **返回值**：`(code, resp)`

#### `create_arm_instance(**kwargs)` — 实例创建（ARM 系列）

- **URL**：`POST /api/proxy/resource/instance/arm/scaling?business_id=xxx`
- **必填参数**：
  - `business_id` (str)
  - `app_id` (str)：与拉流相关的 XHD 拉流属性
  - `pod_name` (str)：pod 名称
  - `node_name` (str)：节点名称
  - `content` (str)：JSON 格式的配置内容
- **请求体**：`{"app_id", "pod_name", "node_name", "content"}`
- **返回值**：`(code, resp)`，`resp` 结构：`{"ret": {"code": 0, "msg": "success"}, "data": {}}`

#### `add_image_to_mysql(**kwargs)` — 游戏镜像添加

- **URL**：`POST /cloudrender/cloudgame/add/game/to/mysql?business_id=xxx`
- **必填参数**：`business_id` (str)、`app_id` (str)、`app_version` (str)、`resolution_level` (int)、`apk_name` (str)、`game_image_id` (str)、`provider` (str)
- **可选参数**：`extra_properties` (str，JSON 格式)
- **请求体**：`{"app_id", "app_version", "resolution_level", "apk_name", "game_image_id", "provider"[, "extra_properties"]}`
- **返回值**：`(code, resp)`

#### `scale_resources(**kwargs)` — 机房维度扩缩容

- **URL**：`POST /cloudrender/cloudgame/resources/scale/instances`
- **必填参数**：`appid` (str)、`version` (str)、`res` (int)、`region` (str)、`label` (str)、`num` (int)、`provider` (str)
- **可选参数**：`spec` (int，默认 `2`)
- **请求体**：`{"appid", "version", "res", "region", "label", "num"[, "spec"]}`
- **返回值**：`(code, resp)`

#### `batch_scale_resources(**kwargs)` — 大区维度扩缩容

- **URL**：`POST /cloudrender/cloudgame/resources/scale/batch`
- **必填参数**：`apps` (list)：扩容列表
- **请求体**：`{"apps": [...]}`
- **返回值**：`(code, resp)`

#### `update_resources(**kwargs)` — 滚动升级

- **URL**：`POST /cloudrender/cloudgame/resources/scale/batch`
- **必填参数**：`appid` (str)、`version` (str)
- **可选参数**：`label` (str)、`is_reuse_old_version` (bool)
- **请求体**：`{"appid", "version"[, "label", "is_reuse_old_version"]}`
- **返回值**：`(code, resp)`

#### `get_game_config(**kwargs)` — 获取游戏具体配置

- **URL**：`GET /cloudrender/cloudgame/appid_infos?appid=xxx[&label=xxx]`
- **必填参数**：`appid` (str)
- **可选参数**：`label` (str)
- **返回值**：`(code, resp)`

#### `initialize_new_game(**kwargs)` — 初始化新游戏配置

- **URL**：`POST /cloudrender/cloudgame/configs/initialize/new_game`
- **必填参数**：`appid` (str)、`version` (str)、`is_save_user_info` (bool)
- **可选参数**：`keep` (任意)
- **请求体**：`{"appid", "version", "is_save_user_info"[, "keep"]}`
- **返回值**：`(code, resp)`

#### `replace_label(**kwargs)` — 游戏配置 label 替换

- **URL**：`POST /cloudrender/cloudgame/resources/scale/replace_label`
- **必填参数**：`old_label` (str)、`new_label` (str)
- **可选参数**：`exclude_app_v` (list)
- **请求体**：`{"old_label", "new_label"[, "exclude_app_v"]}`
- **返回值**：`(code, resp)`

#### `delete_game_configs(**kwargs)` — 删除游戏配置

- **URL**：`POST /cloudrender/cloudgame/resources/scale/del_configs`
- **必填参数**：`appid_infos` (list)，每项必须包含 `app_v`、`num`、`res`、`region`、`label`、`spec`
- **请求体**：`{"appid_infos": [...]}`
- **返回值**：`(code, resp)`

#### `query_app_resources(**kwargs)` — 查询应用资源分布

- **URL**：`GET /cloudrender/cloudgame/resources`
- **必填参数**：`appid` (str)、`version` (str)、`application_type` (int)、`resolution` (int)、`region` (str)
- **请求体**：`{"appid", "version", "resolution", "region", "application_type"}`
- **返回值**：`(code, resp)`，`resp` 结构：
  ```json
  {"ret": 0, "msg": "", "quota_info": {"region": "...", "cfg_quota": 100, "quota": 50, "ready_quota": 20, "free_quota": 20, "busy_streamer_quota": 10, "whitelist_quota": 5, "other_quota": 5}}
  ```
  - ✅ 可用字段：`resp["quota_info"]["free_quota"]`、`["ready_quota"]`、`["quota"]`

#### `query_all_game_capacities(**kwargs)` — 查询所有游戏容量

- **URL**：`GET /cloudrender/cloudgame/resources_v2?query=encodeParam`
- **必填参数**：`business_id` (str)、`application_type` (int，固定 `0`)、`version` (str，固定 `"latest"`)
- **可选参数**：`resolution` (int)、`appids` (list)
- **请求体**：`{"business_id", "application_type", "version"[, "resolution", "appids"]}`
- **返回值**：`(code, resp)`，`resp` 结构：
  ```json
  {"ret": 0, "msg": "ok", "business_id": 1000, "application_type": 0, "quota_info": [{"appid": "...", "resolution": 1, "region": "...", "cfg_quota": 100, "quota": 100, "ready_quota": 100, "free_quota": 100, "busy_streamer_quota": 0, "whitelist_quota": 0, "other_quota": 0}]}
  ```
  - ✅ 可用字段：`resp["quota_info"]` 列表，每项含 `appid`、`resolution`、`region`、`free_quota`、`ready_quota`

#### `get_x86_resources(**kwargs)` — 获取 x86 资源信息

- **URL**：`GET /cloudrender/cloudgame/decrible/arm_x86/service/instance/by/region_id`
- **必填参数**：`business_id` (str)、`region_id` (str)、`provider` (str)
- **可选参数**：`customer_id` (str，默认 `"ieg"`，可选 `"teg"`/`"ieg"`/`"mix"`)
- **请求体**：`{"business_id", "region_id", "provider", "customer_id"}`
- **返回值**：`(code, resp)`，`resp` 结构：
  ```json
  {"ret": {"code": 0, "msg": "success"}, "body": {"x86_list": [{"private_ip": "...", "tcp_start": 1, "tcp_end": 6000, "udp_start": 2, "udp_end": 6000, "group": 1, "region_id": "...", "customer_id": "ieg"}]}}
  ```
  - ✅ 可用字段：`resp["body"]["x86_list"]`，每项含 `private_ip`、`region_id`

---

### 精彩视频（来自 `op_wx_video.py`）

#### `get_reuse_app_version_start_num(**kwargs)` — 获取复用版本和启动次数

- **URL**：`POST /cloudrender/test/tools/get_reuse_app_version_start_num`
- **必填参数**：`reuse_appversion_key` (str)
- **请求体**：`{"reuse_appversion_key": ...}`
- **返回值**：`(code, resp)`

---

## Resource_Svr_Request API（`libs/api/resource_svr_api.py`）

> ⚠️ **调用方向说明**：`resource_svr_api.py` 中的方法按调用方向分为多组，**必须严格按照测试步骤描述的调用方向选择对应方法**，不得混用。

### proxy → resource-service 接口

| 方法 | 说明 | URL | 必填参数 |
|------|------|-----|---------|
| `get_instance_addr_info(**kwargs)` | 获取实例地址信息 | `GET api/proxy/instance/addr/search?instance_id=xxx` | `instance_id` (str) |
| `mount_game_image(**kwargs)` | 挂载游戏 | `POST api/proxy/resource/mount/game/image` | `instance_ids` (list)；可选：`app_id`、`app_version`、`resolution_level`、`game_image_id`、`provider`、`business_id` |
| `app_expansion_proxy(**kwargs)` | 扩容 | `POST api/proxy/app/expansion` | `app_id`、`version`、`total`、`business_id`、`region_id`、`provider`、`source`、`user`、`region` |
| `proxy_scaling_instance(**kwargs)` | 缩容实例 | `POST api/proxy/instance/scaling` | `instance_id` (str) |
| `release_instance(**kwargs)` | 回收实例 | `POST api/proxy/instance/release` | `instance_id` (str) |
| `search_instance_by_task(**kwargs)` | 根据 task_id 查询实例 | `POST api/proxy/instance/search` | `task_id` (str)；可选：`pageno`(默认1)、`count`(默认200) |
| `proxy_reset_instance(**kwargs)` | 重置实例 | `POST api/proxy/instance/reset` | `instance_ids` (list) |
| `generate_cos_key(**kwargs)` | 获取 cos 临时密钥 | `POST api/proxy/generate/cos/key` | `business_id`、`duration_seconds`、`allow_prefix`、`allow_actions` |
| `generate_download_link(**kwargs)` | 获取云端临时下载链接 | `POST api/proxy/generate/download/link` | `key`、`business_id`、`expired` |
| `create_game_image(**kwargs)` | 插入游戏镜像 | `POST api/proxy/gameimages/create` | `app_id`、`app_version`、`provider`、`resolution_level`、`apk_name`、`game_image_id`、`height`、`width`、`extra_properties`、`business_id` |
| `filter_instances_count(**kwargs)` | 获取各机房实例数目 | `POST api/proxy/instances/filter/count` | `filter` (dict)；可选：`pageno`、`count` |
| `filter_game_images_search(**kwargs)` | 获取 provider 与 appid 对应信息 | `POST api/proxy/game_images/filter/search` | `filter` (dict)；可选：`pageno`、`count` |
| `search_template_instances(**kwargs)` | 获取模板实例信息 | `POST api/proxy/template_instances/search` | 可选：`pageno`、`count` |
| `search_provider_configs(**kwargs)` | 获取第三方配置 | `POST api/proxy/provider_configs/search` | 可选：`pageno`、`count` |
| `search_business_id_configs(**kwargs)` | 获取 business_id 配置 | `POST api/proxy/business_id_configs/search` | 可选：`pageno`、`count` |
| `create_cloud_game_image_info(**kwargs)` | 插入 cloud_game_image_infos 表 | `POST api/proxy/cloud_game_image_infos/create` | `cloud_id`、`cloud_path`、`bucket`、`business_id`、`provider`、`instance_id` |
| `query_cloud_game_image_info(**kwargs)` | 获取 cloud_path | `POST api/proxy/cloud_game_image_infos/query` | `cloud_id`、`business_id` |
| `create_gameimage_cloudpath_map(**kwargs)` | 插入 gameimage_cloudpath_map | `POST api/proxy/gameimage_cloudpath_map/create` | `cloud_id`、`business_id`、`provider`、`game_image_id` |
| `search_server_id_map(**kwargs)` | 获取 linux_id | `POST api/proxy/server_id/map/search` | `server_id` (str) |
| `search_instance_host(**kwargs)` | 获取实例 host_ip | `GET api/proxy/instance/host/search/?instance_id=xxx` | `instance_id` (str) |
| `search_region_id(**kwargs)` | 获取 region_id 信息 | `POST api/cgmanager/region/id/search` | `region_id`；可选：`pageno`、`count` |

#### `mount_game_image`（resource_svr）返回值

```json
{"ret": 0, "msg": "success"}
```
- ✅ 可用字段：`resp.get("ret")`、`resp.get("msg")`

### cgmanager → resource-service 接口

| 方法 | 说明 | URL | 必填参数 |
|------|------|-----|---------|
| `report_flag(**kwargs)` | 实例 flag 修改 | `POST api/cgmanager/report/flag` | `data_list` (list) 或 `instance_ids`(list)+`flag`(int，默认1) |
| `report_app_info(**kwargs)` | 修改实例 app_id/app_version/resolution | `POST api/cgmanager/report/app_info` | `data_list` 或 `instance_ids`+`app_id`+`app_version`；可选：`resolution`(默认1)、`flag`(默认1) |
| `report_host_ip(**kwargs)` | 修改实例 host_ip | `POST api/cgmanager/report/host_ip` | `data_list` 或 `instance_ids`(list)+`host_ip`(str) |
| `set_instance_attribute(**kwargs)` | 修改实例属性 | `POST api/cgmanager/instance/attribute/set` | `instance_id_list`(list)、`video_resolution`、`xhd_version` |
| `search_instance_by_ip(**kwargs)` | 根据 ip 查询实例 | `POST api/cgmanager/instance/search` | `host_ip` (str)；可选：`pageno`(默认1)、`count`(默认200) |
| `apply_instance_by_server_id(**kwargs)` | 申请同张板卡的空闲实例 | `POST api/cgmanager/instance/server_id/apply` | `server_id`、`flag`、`host_ip` |
| `apply_instance(**kwargs)` | 申请指定个数的空闲实例 | `POST api/cgmanager/instance/apply` | `region_id`、`spec`、`business_id`、`flag`、`host_ip`、`modify_number` |
| `clean_instance(**kwargs)` | 清除实例的绑定关系 | `POST api/cgmanager/instance/clean` | `instance_id` (str) |
| `bind_instance(**kwargs)` | 修改同张板卡实例的绑定关系 | `POST api/cgmanager/instance/bind` | `server_id`、`host_ip`、`app_id`、`app_version`、`flag` |
| `heartbeat_report(**kwargs)` | 上报心跳 | `POST api/cgmanager/heartbeat/report` | `data_list` 或 `instance_id`+`channel_id`+`status`+`width`+`height`+`app_id`+`app_version`+`flag`+`host_ip`+`resolution` |

### admin(工具) → resource-service 接口

| 方法 | 说明 | URL | 必填参数 |
|------|------|-----|---------|
| `create_instance_admin(**kwargs)` | 创建实例 | `POST api/admin/instance/create` | `data_list` 或 `instance_id_list`+`video_resolution`+`xhd_version` |
| `search_instance(**kwargs)` | 查询实例列表 | `POST api/admin/instance/search` | 可选：`host_ip`、`instance_id`、`region_id`、`provider`、`app_id`、`flag`、`app_version`、`pageno`(默认1)、`count`(默认200) |
| `scaling_instance(**kwargs)` | 缩容实例 | `POST api/admin/instance/scaling` | `host_ip_list`(list)、`instance_id_list`(list) |
| `recovery_instance(**kwargs)` | 回收实例 | `POST api/admin/instance/recovery` | `host_ip_list`(list)、`instance_id_list`(list) |
| `reset_instance(**kwargs)` | 重置实例 | `POST api/admin/instance/reset` | `instance_ids` (list) |
| `app_expansion_admin(**kwargs)` | 应用扩容 | `POST api/admin/app/expansion` | `app_id`、`version`、`total`、`business_id`、`region_id`、`provider`、`source`、`user`、`region` |
| `search_task(**kwargs)` | 查询任务 | `POST api/admin/task/search` | 可选：`id`、`type`、`app_id`、`version`、`status`、`source`、`user`、`business_id`、`region_id`、`pageno`、`count` |
| `modify_task(**kwargs)` | 修改任务 | `POST api/admin/task/modify` | `id`、`status`、`create_count`、`source`、`user` |
| `search_region(**kwargs)` | 查询 region | `POST api/admin/region/search` | 可选：`region_id`、`region`、`provider`、`pageno`、`count` |
| `modify_region(**kwargs)` | 修改 region | `POST api/admin/region/modify` | `region_id`、`region`、`provider` |
| `delete_region(**kwargs)` | 删除 region | `POST api/admin/region/delete` | `region_id` (str) |
| `create_region(**kwargs)` | 新增 region | `POST api/admin/region/create` | `region_id`、`region`、`provider` |
| `search_cgmanager_configs(**kwargs)` | 获取 cg_manager 表数据 | `POST api/admin/cgmanager/configs/search` | 可选：`ip`、`pageno`、`count` |
| `add_cgmanager_configs(**kwargs)` | 向 cg_manager 表新增数据 | `POST api/admin/cgmanager/configs/add` | `ip_list`、`private_ip_list`、`enable_database`、`enable_scale`、`enable_reboot`、`enable_update`、`enable_local_scale`、`enable_process_error`、`enable_local_third_api`、`enable_set_private_ip`、`reboot_start_time`、`reboot_end_time`、`max_num`、`cpu_count`、`para_mount_count`、`spec`、`provider`、`business_id`、`region_id`、`image_id`、`cgmanager_config` |
| `modify_cgmanager_configs(**kwargs)` | 修改 cg_manager 表内容 | `POST api/admin/cgmanager/configs/modify` | `ip`（其余同 add） |
| `delete_cgmanager_configs(**kwargs)` | 删除 cg_manager 数据 | `POST api/admin/cgmanager/configs/delelte` | `ip_list` (list) |
| `search_game_images(**kwargs)` | 获取 game_images 表记录 | `GET api/cgmanager/images/game/search` | 可选：`app_id`、`app_version`、`resolution_level`、`provider`、`business_id`、`pageno`、`count` |
| `search_provider_identification(**kwargs)` | 获取指定 provider 鉴权信息 | `GET api/cgmanager/provider/identification/search` | 可选：`provider`、`business_id`、`pageno`、`count` |

> ⚠️ **注意**：`set_session_id` 方法已弃用（用于根据 session_id 录像），禁止在新代码中使用。

### k8s 模块 → resource-service 接口

| 方法 | 说明 | URL | 必填参数 |
|------|------|-----|---------|
| `k8s_create_instance(**kwargs)` | 创建实例 | `POST api/k8s/instance/create` | `region_id`、`host_ip`、`instance_id`、`flag`、`app_id`、`app_version`、`provider`、`business_id` |
| `k8s_delete_instance(**kwargs)` | 删除实例 | `POST api/k8s/instance/delete` | `instance_id` (str) |

---

## CG_Manager_Request API（`libs/api/cgmanager_api.py`）

> **默认端口**：32767，URL 前缀：`cloudrender/cloudgame`
> **特殊说明**：支持多 IP 模式（`ip` 传 list），`both_req=True` 时对所有 IP 都发请求，返回 `(code_dict, resp_dict)`；单 IP 时返回 `(code, resp)`

| 方法 | 说明 | URL | 必填参数 | 返回值 |
|------|------|-----|---------|--------|
| `aic_show(**kwargs)` | 容器命令行输入 | `POST cloudrender/cloudgame/offline/aic_sh` | `channel_id` (str)、`aic_sh_cmd` (str) | `(code, resp)` |
| `free_instances(**kwargs)` | 获取空闲实例 | `POST cloudrender/cloudgame/free_instances` | 无 | `(code, resp)` |
| `non_origin_instances(**kwargs)` | 获取所有实例 | `POST cloudrender/cloudgame/non_origin_instances` | 无 | `(code, resp)` |
| `get_route_info_by_instance_id(**kwargs)` | 获取路由信息 | `POST cloudrender/cloudgame/get_route_info_by_instance_id` | `instance_id` (str) | `(code, resp)` |
| `online_start_game(**kwargs)` | 启动游戏（精彩视频） | `POST cloudrender/cloudgame/online/start_game` | `file_json` (dict) 或直接传 kwargs | `(code, resp)` |

---

## Master_Request API（`libs/api/master_api.py`）

> **默认端口**：10086，URL 前缀：`api`

| 方法 | 说明 | URL | 必填参数 | 返回值 |
|------|------|-----|---------|--------|
| `get_master_info(**kwargs)` | 获取 master 信息【mock 接口，无实际用途】 | `GET api/master/instance/addr/search?instance_id=xxx` | `instance_id` (str)；若为空则直接返回 `(000, None)` | `(code, resp)` |

---

## Game_Oss_Request API（`libs/api/oss_api.py`）

> **默认端口**：80，URL 前缀：`ins_manager`
> **特殊说明**：请求体使用 `application/x-www-form-urlencoded` 编码，数据经过 URL encode 处理

### 实例管理接口

| 方法 | 说明 | URL | 必填参数 |
|------|------|-----|---------|
| `instance_up(**kwargs)` | 实例上架 | `POST ins_manager/instance_up_down/up/0` | `ip_list` (list) |
| `instance_down(**kwargs)` | 实例下架 | `POST ins_manager/instance_up_down/down/0` | `ip_list` (list) |
| `instance_reset(**kwargs)` | 实例重置 | `POST ins_manager/instance_up_down/reset/0` | `ip_list` (list) |
| `instance_delete(**kwargs)` | 实例删除 | `POST ins_manager/instance_up_down/del/0` | `ip_list` (list) |
| `instance_recover(**kwargs)` | 实例恢复 | `POST ins_manager/instance_up_down/ON/0` | `ip_list` (list) |
| `instance_isolate(**kwargs)` | 实例隔离 | `POST ins_manager/instance_up_down/OFF/0` | `ip_list` (list) |
| `instance_mount(**kwargs)` | 实例重复挂载 | `POST ins_manager/instance_up_down/mount/0` | `ip_list` (list) |
| `instance_blank(**kwargs)` | 实例置为空闲状态 | `POST ins_manager/instance_up_down/blank/0` | `ip_list` (list) |

> 以上接口均有可选参数 `remark` (str，默认 `""`)

#### `instance_remount(**kwargs)` — 参数挂载

- **URL**：`POST ins_manager/instance_remount/remount/0`
- **必填参数**：`ip_list` (list)、`app_id` (str)、`app_version` (str)
- **可选参数**：`resolution` (str，默认 `"1"`)、`remark` (str，默认 `"1"`)
- **请求体**：`{"ip_list", "app_id", "app_version", "resolution", "remark"}`
- **返回值**：`(code, resp)`

### 扩缩容接口（通过 resource-service）

#### `instance_expansion_by_rs(**kwargs)` — 扩容实例

- **URL**：`POST ins_manager/scale_up_instances_by_resource_service/`
- **必填参数**：`app_id` (str)、`version` (str)、`total` (int)、`business_id` (str)、`region_id` (str)、`env_prod` (str)
- **返回值**：`(code, resp)`

#### `instance_reduction_by_rs(**kwargs)` — 缩容实例

- **URL**：`POST ins_manager/scale_instances_by_resource_service/`
- **必填参数**：`ip_list` (str)
- **可选参数**：`instance_id_list` (str，默认 `""`)、`env_prod` (str，默认 `"0"`)
- **返回值**：`(code, resp)`

#### `instance_recovery_by_rs(**kwargs)` — 回收实例

- **URL**：`POST ins_manager/recovery_instances_by_resource_service/`
- **必填参数**：`ip_list` (str)
- **可选参数**：`instance_id_list` (str，默认 `""`)、`env_prod` (str，默认 `"0"`)
- **返回值**：`(code, resp)`

#### `instance_reset_by_rs(**kwargs)` — 重置实例

- **URL**：`POST ins_manager/recovery_instances_by_resource_service/`
- **必填参数**：`instance_id_list` (list)
- **可选参数**：`env_prod` (str，默认 `"0"`)
- **返回值**：`(code, resp)`

### 扩容配置接口

#### `instance_scale_by_label(**kwargs)` — 扩容管理增加

- **URL**：`POST ins_manager/scale_by_label/`
- **必填参数**：`appid` (str)、`version` (str)、`num` (int)、`label` (str)
- **可选参数**：`env_prod` (str，默认 `"0"`)、`res` (str，默认 `"1"`)、`region` (str)、`business_id` (str，默认 `"1000"`)、`spec` (str)
- **返回值**：`(code, resp)`

#### `instance_delete_scale_config(**kwargs)` — 扩容管理删除

- **URL**：`POST ins_manager/delete_scale_config/`
- **必填参数**：`appid_infos` (str)
- **可选参数**：`business_id` (str，默认 `"1000"`)、`env_prod` (str，默认 `"0"`)
- **返回值**：`(code, resp)`

#### `instance_get_appid_info(**kwargs)` — 扩容管理查询

- **URL**：`POST ins_manager/get_appid_info/`
- **必填参数**：`appid_infos` (str)
- **可选参数**：`appid`、`business_id`(默认 `"1000"`)、`env_prod`(默认 `"0"`)、`label`、`region`、`res`、`spec`
- **返回值**：`(code, resp)`

### CGManager 表管理接口

#### `update_cgmanager_table(**kwargs)` — 更新 CGManager 表

- **URL**：`POST ins_manager/ins_manager/update_cgmanager_table/`
- **必填参数**：
  - `env_prod` (int)：环境标识
  - `update_list` (list)：更新列表，每项为 dict，必须包含 `ip`(str)、`provider`、`business_id`(str)、`region_id`、`field`(str)、`before_value`(str)、`after_value`(str)
- **返回值**：`(code, resp)`

#### `batch_update_cgmanager_table(**kwargs)` — 批量更新 CGManager 表

- **URL**：`POST ins_manager/ins_manager/batch_update_cgmanager_table/`
- **必填参数**：`env_prod` (int)、`ip_list` (list)
- **可选参数**：`private_ip_list` (str)、`enable` (dict，功能开关)、`other_col` (dict，其他配置)、`business_id` (str)
- **返回值**：`(code, resp)`

#### `del_data_from_cgmanager_table(**kwargs)` — 从 CGManager 表删除数据

- **URL**：`POST ins_manager/ins_manager/del_data_from_cgmanager_table/`
- **必填参数**：`env_prod` (int)、`ip_list` (list)
- **返回值**：`(code, resp)`

#### `add_to_cgmanager_table(**kwargs)` — 向 CGManager 表添加数据

- **URL**：`POST ins_manager/ins_manager/add_to_cgmanager_table/`
- **必填参数**：`env_prod` (int)、`ip_list` (str，每行一个IP)、`private_ip_list` (str，每行一个IP)
- **可选参数**：`enable_database`/`enable_scale`/`enable_reboot`/`enable_update`/`enable_local_scale`/`enable_process_error`/`enable_local_third_api`/`enable_set_private_ip`（均默认 `"1"`）、`business_id`(默认 `"1000"`)、`image_id`、`max_num`、`region_id`、`provider`、`reboot_start_time`、`reboot_end_time`、`cpu_count`、`spec`、`para_mount_count`、`cgmanager_config`、`status`
- **返回值**：`(code, resp)`

---

## Game_Stream_Request API（`libs/api/cdn_api.py`）

> **初始化**：`Game_Stream_Request(cdn_ip, cdn_host, cdn_port=80)`
> **URL 格式**：`http://{cdn_ip}:{cdn_port}/{cdn_host}/webrtc/v1/{action}`
> **特殊说明**：请求头中 `Host` 固定为 `cdn_host`

#### `pullstream(**kwargs)` — 游戏推流

- **URL**：`POST webrtc/v1/pullstream`
- **必填参数**：`streamurl` (str)、`sessionid` (str)、`localsdp` (str)
- **可选参数**：`clientip` (str)
- **请求体**：`{"streamurl", "sessionid", "clientip", "localsdp"}`
- **返回值**：`(code, resp)`

#### `stopstream(**kwargs)` — 停止推流

- **URL**：`POST webrtc/v1/stopstream`
- **必填参数**：`streamurl` (str)
- **请求体**：`{"streamurl"}`
- **返回值**：`(code, resp)`

---

## Docker_Midserver_Request API（`libs/api/docker_midserver_api.py`）

> **初始化**：`Docker_Midserver_Request(ip, test_ip, port=2375)`
> **特殊说明**：通过 SSH 远程执行 curl 命令，返回值为 `(ret, out, err)` 而非 `(code, resp)`

#### `container_restart(**kwargs)` — 容器重启

- **URL**：`POST task/container_restart`
- **必填参数**：`id` (str)、`vol` (str)、`instance_id` (str)、`biz_id` (str)、`game_image_id` (str)
- **可选参数**：`resolution` (str，默认 `""`)、`trace_id` (str，默认 `""`)、`force` (str，默认 `""`)、`extra_properties` (任意)
- **请求体**：`{"id", "vol", "instance_id", "biz_id", "game_image_id"[, "resolution", "trace_id", "force", "extra_properties"]}`
- **返回值**：`(ret, out, err)`
  - `ret` (bool)：命令是否执行成功
  - `out` (str)：标准输出
  - `err` (str)：错误输出

> ⚠️ **注意**：`Docker_Midserver_Request` 的返回值是 `(ret, out, err)` 三元组，与其他 API 的 `(code, resp)` 不同，使用时必须注意区分。

---

## 关键字函数（`libs/mytest/keywords_api.py`）

> ⚠️ **优先级规则**：生成测试逻辑时，**必须优先从 `keywords_api.py` 中查找可复用的关键字函数**，避免在用例中重复实现已有逻辑。

### 验证类

| 函数 | 说明 |
|------|------|
| `validate_results(expected_and_operations, actual_results)` | 多字段结果验证 |
| `validate_thread_ret(thread_ret, expected_results, ignore_first=True)` | 验证多线程拉流结果 |
| `validate_all_match(ex_ret, ac_ret, check_op='Res')` | 验证两个结果集完全匹配 |
| `assert_validate_log(assert_ret, assert_print_log, assert_key=None)` | 生成断言失败时的详细日志 |

### 实例状态类

| 函数 | 说明 |
|------|------|
| `get_instance_status(ip, instance_id)` | 获取实例状态 |
| `get_instance_status_req(wan_ip, instance_id)` | 通过请求获取实例状态 |
| `get_instance_info_req(wan_ip, instance_id)` | 通过请求获取实例详情 |
| `get_all_instance_req(ip_port_list)` | 获取所有实例信息 |
| `get_free_instance_req(ip_port, instance_id)` | 获取空闲实例 |
| `check_instance_is_free(param, instance_id, is_free=True, timeout=100)` | 轮询检查实例是否空闲 |
| `check_instance_status(ip, instance_id, expected_status=None, unexpected_status=None, timeout=100)` | 轮询检查实例状态 |
| `judge_instance_is_exist(ip, instance_id, is_exist=True, timeout=100)` | 轮询判断实例是否存在 |
| `get_instance_num(ip, instance_id)` | 获取实例数量 |
| `get_instance_info_cmd(ip, instance_id)` | 通过命令获取实例信息 |

### 空闲实例获取类

| 函数 | 说明 |
|------|------|
| `get_free_instance_with_retry(xf_game_id, cg_ip_port, instance_num=1, ...)` | 带重试的空闲实例获取 |
| `get_free_instance_from_mock_svr(xf_game_id, cg_ip_port, instance_num=1, ...)` | 从 mock server 获取空闲实例 |
| `get_all_free_instance_req(ip_port_list)` | 获取所有空闲实例 |
| `get_filter_free_inst(free_instance_list, filter_key)` | 过滤空闲实例列表 |
| `refresh_instance_cache(cg_ip_port, mock_svr_ip)` | 刷新实例缓存 |

### 进程检查类

| 函数 | 说明 |
|------|------|
| `get_process_num(ip, process_name, instance_id)` | 获取进程数量 |
| `judge_process_is_exist(ip, process_name, is_exist=True, timeout=100, ...)` | 轮询判断进程是否存在 |
| `check_process_status(ip, process, pids=None)` | 检查进程状态 |

### 日志分析类

| 函数 | 说明 |
|------|------|
| `analyze_streamer_media_invoke_log(ip, instance_id, session_id, ...)` | 分析 streamer media invoke 日志 |
| `analyze_summary_json(ip, instance_id, log_file, session_id)` | 分析 summary JSON 日志 |
| `analyze_meta_json(ip, instance_id, log_file)` | 分析 meta JSON 日志 |
| `filter_streamer_log(param, instance_id, chid, ...)` | 过滤 streamer 日志 |
| `check_log_fields_exist(log_fields_list, log_file_path)` | 检查日志字段是否存在 |
| `check_push_streamer_is_stop(param, chid, session_id, ...)` | 检查推流是否停止 |
| `set_streamer_log_name(ip, instance_id)` | 设置 streamer 日志名称 |

### 工具类

| 函数 | 说明 |
|------|------|
| `get_chid(instance_info, instance_id)` | 获取 chid |
| `get_chid_map(cg_ip, cmd=None)` | 获取 chid 映射 |
| `get_streamer_info(test_scenes_list, test_streamer)` | 获取 streamer 信息 |
| `upload_download_xhd_file(param, instance_info, dst_xhd, src_path, dst_path)` | 上传/下载 XHD 文件 |
| `ssh_inner_ip_exec_cmd(outer_ip, inner_ip, cmd)` | SSH 内网 IP 执行命令 |
| `kubectl_exec_cmd(action, pod_id, cmd, double_quote)` | K8s 命令执行 |
| `execute_master_hb_client(ip, cli_num, seconds)` | 执行 master 心跳客户端 |
| `calculate_touch_coordinates(coordinate, coordinate_ratio, resolution)` | 计算触控坐标 |
| `deal_datachannle(data, resolution)` | 处理数据通道数据 |
| `check_video_with_ffmpeg(video_path)` | 用 ffmpeg 检查视频 |
| `check_value_unique(test_value)` | 检查值唯一性 |
| `clean_output(text)` | 清理输出文本 |

---

## 拉流工具库（`libs/mytest/`）

| 方法 | 文件 | 说明 |
|------|------|------|
| `StartPlayer.run_main_wait()` | `player_api.py` | 启动→alloc→拉流→等待→stop（默认含 alloc 和 stop） |
| `StartPlayer.run_main_async()` | `player_api.py` | 启动→alloc→拉流（异步，默认含 alloc，不含 stop） |
| `thread_player(...)` | `thread_api.py` | 多实例并发拉流 |
| `multi_thread_player_cmd(...)` | `thread_api.py` | 拉流中发送命令 |
| `remote_run_cmd_batch(cmd_data)` | `remote_api.py` | 批量远程命令执行，返回 `(ret, cmd_result)` |

### `remote_run_cmd_batch` 返回值结构

```python
# 输入
cmd_data = [{"dst_ip": ip, "cmd": cmd_str}]

# 返回值
ret: bool          # 所有命令是否全部成功
cmd_result: list   # 每条命令的执行结果
# cmd_result[i] 结构：
# {
#     "ret": True,       # 该条命令是否成功
#     "out_msg": "",     # ✅ 正确字段：命令标准输出
#     "err_msg": ""      # ✅ 正确字段：命令错误输出
# }

# ✅ 正确用法
ret, cmd_result = remote_run_cmd_batch(cmd_data)
out = cmd_result[0].get("out_msg", "") if ret and cmd_result else ""

# ❌ 错误用法（字段名不存在，永远返回空字符串）
out = cmd_result[0].get("stdout", "")   # 错误！未在文档中列出
out = cmd_result[0].get("output", "")   # 错误！未在文档中列出
```
