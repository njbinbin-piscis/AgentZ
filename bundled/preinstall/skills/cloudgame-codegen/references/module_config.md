# 模块配置与 API 调用规范

> 本文档是**模块配置**和 **API 调用规范**的唯一权威来源，包含：场景标签速查、各模块差异化代码片段、IP 提取规则、API 初始化规则。

## 一、模块总览

| 模块 | 类型标签（唯一） | 场景变量 | 实例标签前缀 | 参数名 |
|------|----------------|---------|------------|--------|
| xhd | `FullLink` / `XHDModule` | `TEST_SCENES` | `ins_` | `param` |
| proxy | `ProxyModule` | `TEST_PROXY_SCENES` | `proxy_` | `proxy_param` |
| master | `MasterModule` | `TEST_MASTER_SCENES` | `master_` | `master_param` |
| resource_svr | `ResourceSvrModule` | `TEST_RESOURCE_SCENES` | `rs_` | `rs_param` |
| cgmanager | `CGManagerModule` | `TEST_CGMANAGER_SCENES` | `cg_` | `cg_param` |
| wx_video | `CGManagerModule` | `TEST_WV_VIDEO_SCENES` | — | `wv_param` |

## 二、场景标签速查

> ⚠️ 类型标签只能选一个（互斥），场景标签必须属于该类型对应的可用列表。

| 模块 | 类型标签（只能选一） | 可用场景标签 |
|------|-------------------|------------|
| xhd | `FullLink` 或 `XHDModule` | `XHDPullStream`, `XHDConfig`, `XHDReliability`, `XHDInstanceOp`, `Basic` |
| proxy | `ProxyModule` | `ProxyApi`, `ProxyConfig`, `ProxyReliability`, `ProxyFunction`, `ProxyImage` |
| master | `MasterModule` | `MasterApi`, `MasterConfig`, `MasterReliability`, `MasterHeartbeat`, `MasterFunction` |
| resource_svr | `ResourceSvrModule` | `ResourceSvrApi`, `ResourceSvrConfig`, `ResourceSvrReliability`, `ResourceSvrFunction` |
| cgmanager | `CGManagerModule` | `CGApi`, `CGConfig`, `CGReliability`, `CGFunction`, `CGWVideo` |
| wx_video | `CGManagerModule` | `WVVideo` |

## 三、IP 提取与 API 初始化规则

各模块 IP 字段格式不同，提取方式和 API 初始化代码如下：

| 模块 | IP 提取来源 | IP 提取代码 | API 初始化代码 |
|------|------------|------------|--------------|
| proxy | `proxy_param["PROXY_INFO"]["ip"][0]` | `raw_ip.split(":")[0] if ":" in raw_ip else raw_ip` | `self.ProxyAPI = ProxyAPI(proxy_ip)` |
| master | `master_param["MASTER_INFO"]["ip"][0]` | `raw_ip.split(":")[0] if ":" in raw_ip else raw_ip` | `self.MasterAPI = Master_Request(master_ip)` |
| resource_svr | `rs_param["RESOURCE_INFO"]["lan_ip"][0]` | `raw_ip.split(":")[0] if ":" in raw_ip else raw_ip` | `self.RSAPI = Resource_Svr_Request(rs_ip)` |
| cgmanager | `cg_param["CGMANAGER_INFO"]["wan_ip"]` | 直接使用，无需分割 | `self.CGAPI = CG_Manager_Request(cg_ip)` |
| wx_video | `wv_param["WV_VIDEO_INFO"]["cg_ip"]` | 直接使用，无需分割 | `self.CGAPI = CG_Manager_Request(cg_ip)` |

### 模板 B/C 中 API 访问优先级规则

> ⚠️ **重要**：模板 B/C 中，`self.*API` 的初始化遵循以下优先级规则：
>
> **① 当测试函数内有对应的 `self.<模块>_param` 变量时**，必须从该变量中提取 IP 来初始化对应 API，**不能**直接使用 `self.param.ProxyAPI` 等（那是模板 A 专属）。
>
> **② 当需要用到其他模块的 API，且该模块没有对应的 `self.<模块>_param` 变量时**，则使用 `self.param.ProxyAPI`、`self.param.OSSAPI`、`self.param.RSAPI`、`self.param.CGAPI`、`self.param.DMServerAPI` 来访问（来自 `TEST_SCENES`）。

### API 调用路由

```
xhd 模块（模板A）?
├── 是 → 通过 self.param 直接访问（来自 TEST_SCENES）：
│        self.param.ProxyAPI      # proxy 接口
│        self.param.OSSAPI        # OSS 接口
│        self.param.RSAPI         # resource_svr 接口
│        self.param.CGAPI         # cgmanager 接口
│        self.param.DMServerAPI   # docker midserver 接口
└── 否（模板B/C）→ 用模块 IP 初始化 API 对象（主模块）：
    ├── proxy        → self.ProxyAPI = ProxyAPI(proxy_ip)
    ├── resource_svr → self.RSAPI = Resource_Svr_Request(rs_ip)
    ├── cgmanager    → self.CGAPI = CG_Manager_Request(cg_ip)
    └── master       → self.MasterAPI = Master_Request(master_ip)
    ⚠️ 若还需调用其他模块，仍可通过 self.param.OSSAPI 等访问（见上方优先级规则）
```

## 四、步骤描述 → API 对象严格对应规则

> 🚫 **铁律**：**测试步骤文字描述是唯一判断依据**，URL 路径、方法名、个人理解均不能作为判断依据。

| 测试步骤描述 | 模板A 必须使用 | 模板B/C 必须使用 | 禁止替换为 |
|------------|-------------|----------------|----------|
| 通过 **Proxy** / 调用 Proxy 接口 | `self.param.ProxyAPI` | `self.ProxyAPI` | ~~`self.RSAPI`~~、~~`self.CGAPI`~~ |
| 通过 **ResourceServer** / 调用 RS 接口 | `self.param.RSAPI` | `self.RSAPI` | ~~`self.ProxyAPI`~~、~~`self.CGAPI`~~ |
| 通过 **CGManager** / 调用 CG 接口 | `self.param.CGAPI` | `self.CGAPI` | ~~`self.RSAPI`~~、~~`self.ProxyAPI`~~ |
| 通过 **OSS** / 调用 OSS 接口 | `self.param.OSSAPI` | `self.OSSAPI` | ~~其他 API 对象~~ |
| 通过 **DMServer** / 调用 DM 接口 | `self.param.DMServerAPI` | `self.DMServerAPI` | ~~其他 API 对象~~ |
| 通过 **Master** / 调用 Master 接口 | `self.param.MasterAPI` | `self.MasterAPI` | ~~其他 API 对象~~ |

**典型混淆场景（必须避免）：**

> ⚠️ **URL 路径含模块名 ≠ 使用该模块的 API 对象**
>
> 例如：`resource_svr_api.py` 中存在 URL 为 `proxy/resource/mount/game/image` 的方法 `mount_game_image`，但这**不代表**可以用 `RSAPI` 来执行"通过 proxy 触发"的步骤。
>
> - ❌ 错误：步骤写"通过 proxy 触发挂载任务"，代码用了 `self.param.RSAPI.mount_game_image(...)`
> - ✅ 正确：步骤写"通过 proxy 触发挂载任务"，代码必须用 `self.param.ProxyAPI.instance_level_mounting(...)`

## 五、各模块差异化代码片段

以下仅展示每个模块**区别于通用模板的部分**。通用骨架见 [test_template.py](test_template.py)。

### XHD（模板 A）

```python
@pytest.mark.ins_2
@pytest.mark.parametrize(
    "param",
    filter_testcases(TEST_SCENES, ENV_INFO=XF_AOSP10_865 + XF_AOSP11_VAST + XF_AOSP13_8550)
)
def test_xxx(self, param, step_logger):
    self.param = param
    self.stop_flag = False

    step_logger("并发拉流")
    thread_ret = thread_player(self.param, INSTANCE_INFO, self.instance_list, cost_time=33, dump_data=True)
    check_ret, check_info = validate_thread_ret(thread_ret, expected_results)
    assert check_ret, assert_validate_log(check_ret, check_info)
```

**teardown 示例**：
```python
def teardown_method(self, method):
    if hasattr(self, "stop_flag") and self.stop_flag:
        self.param.ProxyAPI.stop_game(session_id=self.session_id["sessionid"])
```

### Proxy（模板 B）

```python
@pytest.mark.ProxyModule
@pytest.mark.ProxyApi
@pytest.mark.proxy_1
@pytest.mark.ins_1
@pytest.mark.parametrize("proxy_param", TEST_PROXY_SCENES)
@pytest.mark.parametrize("param", filter_testcases(TEST_SCENES, ENV_INFO=XF_AOSP13_8550))
def test_xxx(self, proxy_param, param, step_logger):
    self.param = param
    self.proxy_param = proxy_param
    self.cleanup_flag = False

    step_logger("获取 Proxy IP 并初始化 API")
    raw_ip = self.proxy_param["PROXY_INFO"]["ip"][0]
    proxy_ip = raw_ip.split(":")[0] if ":" in raw_ip else raw_ip
    self.ProxyAPI = ProxyAPI(proxy_ip)
```

### Master（模板 B）

```python
@pytest.mark.MasterModule
@pytest.mark.MasterApi
@pytest.mark.master_1
@pytest.mark.ins_1
@pytest.mark.parametrize("master_param", TEST_MASTER_SCENES)
@pytest.mark.parametrize("param", filter_testcases(TEST_SCENES, ENV_INFO=XF_AOSP13_8550))
def test_xxx(self, master_param, param, step_logger):
    self.master_param = master_param
    self.param = param
    self.cleanup_flag = False

    step_logger("获取 Master IP 并初始化 API")
    raw_ip = self.master_param["MASTER_INFO"]["ip"][0]
    master_ip = raw_ip.split(":")[0] if ":" in raw_ip else raw_ip
    self.MasterAPI = Master_Request(master_ip)
```

### Resource Server（模板 B）

```python
@pytest.mark.ResourceSvrModule
@pytest.mark.ResourceSvrApi
@pytest.mark.rs_1
@pytest.mark.ins_1
@pytest.mark.parametrize("rs_param", TEST_RESOURCE_SCENES)
@pytest.mark.parametrize("param", filter_testcases(TEST_SCENES, ENV_INFO=XF_AOSP13_8550))
def test_xxx(self, rs_param, param, step_logger):
    self.rs_param = rs_param
    self.param = param
    self.cleanup_flag = False

    step_logger("获取 Resource Svr IP 并初始化 API")
    raw_ip = self.rs_param["RESOURCE_INFO"]["lan_ip"][0]
    rs_ip = raw_ip.split(":")[0] if ":" in raw_ip else raw_ip
    self.RSAPI = Resource_Svr_Request(rs_ip)
```

### CGManager（模板 B）

```python
@pytest.mark.CGManagerModule
@pytest.mark.CGApi
@pytest.mark.cg_1
@pytest.mark.ins_1
@pytest.mark.parametrize(
    "cg_param",
    filter_testcases(TEST_CGMANAGER_SCENES, ENV_INFO=[f"cgmanager_{i}" for i in XF_AOSP13_8550])
)
@pytest.mark.parametrize("param", filter_testcases(TEST_SCENES, ENV_INFO=XF_AOSP13_8550))
def test_xxx(self, cg_param, param, step_logger):
    self.cg_param = cg_param
    self.param = param
    self.cleanup_flag = False

    step_logger("获取 CGManager IP 并初始化 API")
    cg_ip = self.cg_param["CGMANAGER_INFO"]["wan_ip"]
    self.CGAPI = CG_Manager_Request(cg_ip)
```

### Proxy（模板 C）

```python
@pytest.mark.ProxyModule
@pytest.mark.ProxyApi
@pytest.mark.proxy_1
@pytest.mark.parametrize("proxy_param", TEST_PROXY_SCENES)
def test_xxx(self, proxy_param, step_logger):
    self.proxy_param = proxy_param
    self.cleanup_flag = False

    step_logger("获取 Proxy IP 并初始化 API")
    raw_ip = self.proxy_param["PROXY_INFO"]["ip"][0]
    proxy_ip = raw_ip.split(":")[0] if ":" in raw_ip else raw_ip
    self.ProxyAPI = ProxyAPI(proxy_ip)
```

### Master（模板 C）

```python
@pytest.mark.MasterModule
@pytest.mark.MasterApi
@pytest.mark.master_1
@pytest.mark.parametrize("master_param", TEST_MASTER_SCENES)
def test_xxx(self, master_param, step_logger):
    self.master_param = master_param
    self.cleanup_flag = False

    step_logger("获取 Master IP 并初始化 API")
    raw_ip = self.master_param["MASTER_INFO"]["ip"][0]
    master_ip = raw_ip.split(":")[0] if ":" in raw_ip else raw_ip
    self.MasterAPI = Master_Request(master_ip)
```

### Resource Server（模板 C）

```python
@pytest.mark.ResourceSvrModule
@pytest.mark.ResourceSvrApi
@pytest.mark.rs_1
@pytest.mark.parametrize("rs_param", TEST_RESOURCE_SCENES)
def test_xxx(self, rs_param, step_logger):
    self.rs_param = rs_param
    self.cleanup_flag = False

    step_logger("获取 Resource Svr IP 并初始化 API")
    raw_ip = self.rs_param["RESOURCE_INFO"]["lan_ip"][0]
    rs_ip = raw_ip.split(":")[0] if ":" in raw_ip else raw_ip
    self.RSAPI = Resource_Svr_Request(rs_ip)
```

### CGManager（模板 C）

```python
@pytest.mark.CGManagerModule
@pytest.mark.CGApi
@pytest.mark.cg_1
@pytest.mark.parametrize(
    "cg_param",
    filter_testcases(TEST_CGMANAGER_SCENES, ENV_INFO=[f"cgmanager_{i}" for i in XF_AOSP13_8550])
)
def test_xxx(self, cg_param, step_logger):
    self.cg_param = cg_param
    self.cleanup_flag = False

    step_logger("获取 CGManager IP 并初始化 API")
    cg_ip = self.cg_param["CGMANAGER_INFO"]["wan_ip"]
    self.CGAPI = CG_Manager_Request(cg_ip)
```

### WX Video（模板 C）

```python
@pytest.mark.CGManagerModule
@pytest.mark.WVVideo
@pytest.mark.cg_1
@pytest.mark.parametrize("wv_param", TEST_WV_VIDEO_SCENES)
def test_xxx(self, wv_param, step_logger):
    self.wv_param = wv_param
    self.cleanup_flag = False

    step_logger("获取 WV IP 并初始化 API")
    cg_ip = self.wv_param["WV_VIDEO_INFO"]["cg_ip"]
    self.CGAPI = CG_Manager_Request(cg_ip)
```

## 六、拉流重复调用风险

**`run_main_wait` 默认行为**：`alloc_op=True`（自动 alloc）+ `stop_op=True`（自动 stop）

> ❌ **禁止重复调用**：如果测试用例中已经**手动调用了 alloc 或 stop 操作**，则**不能**再直接调用默认参数的 `run_main_wait` 或 `run_main_async`。

| 测试用例是否手动调用 alloc/stop | 应使用的拉流方式 |
|-------------------------------|----------------|
| 否（纯拉流测试） | 直接调用 `run_main_wait()` 或 `run_main_async()` |
| 是（已手动 alloc 或 stop） | 必须设置 `alloc_op=False` / `stop_op=False` |

### 手动拉流流程示例

```python
# 方式一：禁用自动 alloc/stop（推荐）
player = StartPlayer(self.param, INSTANCE_INFO, instance_id=xhd_id,
                     cost_time=33, alloc_op=False, stop_op=False)
player.build_test_args(xhd_id, alloc_rsp)
ex_process = player.run_player()
ex_process.wait()
execution_ret, video_ret, audio_ret = player.get_test_ret()

# 方式二：仅禁用自动 alloc，保留自动 stop
player = StartPlayer(self.param, INSTANCE_INFO, instance_id=xhd_id,
                     cost_time=33, alloc_op=False, stop_op=True)
player.get_room_and_cdn(xhd_id, alloc_rsp)
execution_ret, video_ret, audio_ret = player.run_main_wait()
```

### StartPlayer 关键初始化参数

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `alloc_op` | `True` | 是否在拉流前自动调用 alloc |
| `stop_op` | `True` | 是否在拉流后自动调用 stop（仅 `run_main_wait` 使用） |
| `stop_check` | `True` | 是否校验 stop 接口的返回结果 |
| `wait_pull` | `0` | 拉流前等待时间（秒） |
| `slow_stop` | `0` | 拉流结束后延迟停止时间（秒） |
| `sessionid` | 随机生成 | 指定 session_id |
| `userid` | 自动生成 | 指定 user_id |

## 七、动态删除请求 Body 字段（del_key）

> **使用场景**：当测试用例需要验证"缺少某个字段时的接口行为"，可以将该字段的值设为 `"del_key"`，框架会在发送请求前自动将其从 body 中移除。

```python
# 测试缺少 session_id 字段时 stop_game 的返回
status_code, response = self.param.ProxyAPI.stop_game(session_id="del_key")

# 测试缺少 appid 字段时 alloc_instance_v2 的返回
status_code, response = self.param.ProxyAPI.alloc_instance_v2(
    region="biyun-shenzhen-2",
    appid="del_key",          # 此字段将在发送前被移除
    session_id="test_session",
    user_id="test_user",
    instance_id="test_instance",
    business_id=1000
)
```

> ⚠️ `"del_key"` 只对**顶层字段**生效。如需递归删除嵌套结构中的字段，可在 API 方法内手动调用 `recursive_check(data)`（来自 `libs/api/__init__.py`）。

## 八、常用代码模式

### 多值参数化

```python
@pytest.mark.parametrize("touch_info", [
    pytest.param(mult_touch_info, id="mult_touch"),
    pytest.param(mult_add_touch_info, id="mult_add_touch"),
])
def test_example(self, param, step_logger, touch_info):
    pass
```

### 平台分支

```python
if self.param.ENV_INFO in XF_AOSP11_VAST + XF_AOSP11_TIANJI:
    ret, out, err = kubectl_exec_cmd(action="exec_xhd", pod_id=f"xhd-{test_xhd}", cmd=cmd)
else:
    cmd_data = [{"dst_ip": self.param.CGMANAGER_INFO["lan"],
                 "cmd": f"cd /android/cg_manager;./adb_shell.sh {test_xhd} '{cmd}'"}]
    ret, cmd_result = remote_run_cmd_batch(cmd_data)
```

### 环境变量（位于 `libs/mytest/scenes.py`）

| 变量 | 模块 | parametrize 参数名 |
|------|------|-------------------|
| `TEST_SCENES` + `INSTANCE_INFO` | xhd | `param` |
| `TEST_PROXY_SCENES` | proxy | `proxy_param` |
| `TEST_MASTER_SCENES` | master | `master_param` |
| `TEST_RESOURCE_SCENES` | resource_svr | `rs_param` |
| `TEST_CGMANAGER_SCENES` | cgmanager | `cg_param` |
| `TEST_WV_VIDEO_SCENES` | wx_video | `wv_param` |
