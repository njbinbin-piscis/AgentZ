# 场景变量字段速查表

> **用途**：生成测试代码时，如需访问场景变量中的某个字段，**必须先在此处确认字段名是否存在**，禁止编造不存在的字段。
>
> **访问方式**：
> - 模板 A（xhd）：`param.<字段名>` 或 `param.<字段名>['子字段']`
> - 模板 B/C（其他模块）：`<模块参数名>.<字段名>` 或 `<模块参数名>.<字段名>['子字段']`

---

## TEST_SCENES（模板A / 模板B 的 `param` 参数）

> 对应参数名：`param`

| 字段名 | 类型 | 示例值 | 说明 |
|--------|------|--------|------|
| `TEST_PARAM` | bool | `False` | 是否为参数化测试 |
| `ENV_INFO` | str | `'xf_biyun_865'` | 环境标识 |
| `PROVIDER` | str | `'biyun'` | 云服务商 |
| `GAME_ID` | str | `'cjre922fe6h8cgr8'` | 游戏 ID |
| `GET_INS_METHOD` | str | `'specified_instance'` | 获取实例方式 |
| `CDN_INFO` | dict | `{'ip': '183.57.144.39', 'host': '...', 'stun_port': 8000}` | CDN 信息 |
| `CDN_INFO['ip']` | str | `'183.57.144.39'` | CDN IP |
| `CDN_INFO['host']` | str | `'xf-cloudgame.liveplay.myqcloud.com'` | CDN Host |
| `CDN_INFO['stun_port']` | int | `8000` | STUN 端口 |
| `PROXY_INFO` | dict | `{'ip': '29.44.199.145', 'port': 24756}` | Proxy 信息 |
| `PROXY_INFO['ip']` | str | `'29.44.199.145'` | Proxy IP |
| `PROXY_INFO['port']` | int | `24756` | Proxy 端口 |
| `RS_INFO` | dict | `{'ip': '59.36.157.201', 'port': 8080}` | Resource Server 信息 |
| `RS_INFO['ip']` | str | `'59.36.157.201'` | RS IP |
| `RS_INFO['port']` | int | `8080` | RS 端口 |
| `MASTER_INFO` | dict | `{'ip': '9.22.21.203', 'port': 24756}` | Master 信息 |
| `MASTER_INFO['ip']` | str | `'9.22.21.203'` | Master IP |
| `MASTER_INFO['port']` | int | `24756` | Master 端口 |
| `CLIENT_INFO` | dict | `{'ip': '11.141.162.236'}` | 客户端信息 |
| `CLIENT_INFO['ip']` | str | `'11.141.162.236'` | 客户端 IP |
| `CGMANAGER_INFO` | dict | `{'name': None, 'wan': '183.57.144.39', 'lan': '172.16.203.41', 'port': 32767}` | CGManager 信息 |
| `CGMANAGER_INFO['name']` | str/None | `None` | CGManager 名称 |
| `CGMANAGER_INFO['wan']` | str | `'183.57.144.39'` | CGManager 外网 IP |
| `CGMANAGER_INFO['lan']` | str | `'172.16.203.41'` | CGManager 内网 IP |
| `CGMANAGER_INFO['port']` | int | `32767` | CGManager 端口 |
| `INSTANCE_DEV_INFO` | dict | `{'ip': '183.57.144.39'}` | 实例设备信息 |
| `INSTANCE_DEV_INFO['ip']` | str | `'183.57.144.39'` | 实例设备 IP |
| `XHD_INFO` | dict | 见下方展开 | XHD 参数集合 |
| `ProxyAPI` | object | `ProxyAPI(...)` | Proxy 接口对象（直接调用） |
| `OSSAPI` | object | `Game_Oss_Request(...)` | OSS 接口对象（直接调用） |
| `RSAPI` | object | `Resource_Svr_Request(...)` | Resource Server 接口对象 |
| `CGAPI` | object | `CG_Manager_Request(...)` | CGManager 接口对象 |
| `DMServerAPI` | object | `Docker_Midserver_Request(...)` | Docker Midserver 接口对象 |

### XHD_INFO 子字段展开

| 子字段 | 类型 | 示例值 | 说明 |
|--------|------|--------|------|
| `XHD_INFO['business_id']` | str | `'3000'` | 业务 ID |
| `XHD_INFO['client_type']` | str | `'xianfeng'` | 客户端类型 |
| `XHD_INFO['appid']` | str | `'c4bbav66enhpsmqz'` | App ID |
| `XHD_INFO['codec']` | int | `2` | 编解码类型 |
| `XHD_INFO['version']` | str | `'1466081322889486336'` | 版本号 |
| `XHD_INFO['render_timeout']` | int | `60` | 渲染超时（秒） |
| `XHD_INFO['verify_stream']` | int | `0` | 是否验证流 |
| `XHD_INFO['level']` | int | `3` | 等级 |
| `XHD_INFO['resolution']` | int | `1` | 分辨率档位 |
| `XHD_INFO['application_type']` | int | `0` | 应用类型 |
| `XHD_INFO['room']` | str | `'深圳电信宝龙EIC1-60G-GV'` | 房间名 |
| `XHD_INFO['region_id']` | str | `'biyun-foshan-1'` | 区域 ID |
| `XHD_INFO['instance_id']` | list | `['biyun-foshan-1-...', ...]` | 实例 ID 列表 |
| `XHD_INFO['resw']` | int | `1280` | 分辨率宽 |
| `XHD_INFO['resh']` | int | `720` | 分辨率高 |
| `XHD_INFO['res_direction']` | int | `0` | 分辨率方向（0=横屏） |
| `XHD_INFO['fps']` | int | `30` | 帧率 |

---

## INSTANCE_INFO（实例详情字典）

> 访问方式：`INSTANCE_INFO[instance_id]` 取得单个实例字典，或遍历 `.values()`
>
> 通常通过 `self.INSTANCE_INFO` 或 `param.INSTANCE_INFO`（若在 TEST_SCENES 中）访问

| 字段名 | 类型 | 示例值 | 说明 |
|--------|------|--------|------|
| `channel_id` | int | `1526` | 频道 ID |
| `instance_id` | str | `'biyun-shenzhen-2-...'` | 实例 ID |
| `server_id` | str | `'MT367U1000N0'` | 服务器 ID |
| `status` | int | `1` | 实例状态（1=运行中） |
| `region_id` | str | `'biyun-shenzhen-2'` | 区域 ID |
| `ip` | str | `'10.108.10.152'` | 实例内网 IP |
| `width` | int | `1080` | 屏幕宽度（像素） |
| `height` | int | `2280` | 屏幕高度（像素） |
| `spec` | int | `2` | 规格档位 |
| `app_id` | str | `'c4bbav66enhpsmqz'` | App ID |
| `app_version` | str | `'1466094679216984064'` | App 版本号 |
| `resolution` | int | `1` | 分辨率档位 |
| `flag` | int | `1` | 标志位 |
| `mtime` | datetime | `datetime(2026, 2, 21, ...)` | 最后修改时间 |
| `ctime` | datetime | `datetime(2024, 7, 10, ...)` | 创建时间 |
| `host_ip` | str | `'59.36.210.203'` | 宿主机 IP |
| `provider` | str | `'biyun'` | 云服务商 |
| `business_id` | str | `'3000'` | 业务 ID |
| `task_id` | str/None | `None` | 任务 ID |
| `card_type` | str | `''` | 显卡类型 |
| `session_id` | str | `''` | 会话 ID |
| `video_resolution` | int | `1080` | 视频分辨率 |
| `xhd_version` | str | `'JSON_stop_aosp13_...'` | XHD 版本标识 |
| `is_test` | bool | `True` | 是否测试实例 |

---

## TEST_PROXY_SCENES（`proxy_param` 参数）

| 字段名 | 类型 | 示例值 | 说明 |
|--------|------|--------|------|
| `ENV_INFO` | str | `'proxy_dev'` | 环境标识 |
| `PROXY_INFO` | dict | 见下方 | Proxy 详情 |
| `PROXY_INFO['env']` | str | `'Development'` | 环境类型 |
| `PROXY_INFO['ip']` | list | `['29.44.199.147:24756']` | Proxy IP 列表（含端口） |
| `PROXY_INFO['is_test']` | bool | `True` | 是否测试环境 |
| `PROXY_INFO['name']` | str | `'yyb_cloudgame'` | Proxy 服务名 |

---

## TEST_MASTER_SCENES（`master_param` 参数）

| 字段名 | 类型 | 示例值 | 说明 |
|--------|------|--------|------|
| `ENV_INFO` | str | `'master_dev'` | 环境标识 |
| `MASTER_INFO` | dict | 见下方 | Master 详情 |
| `MASTER_INFO['env']` | str | `'Development'` | 环境类型 |
| `MASTER_INFO['ip']` | list | `['29.44.199.147:10086']` | Master IP 列表（含端口） |
| `MASTER_INFO['is_test']` | bool | `True` | 是否测试环境 |
| `MASTER_INFO['name']` | str | `'yuanshen_cloudgame_test_master'` | Master 服务名 |

---

## TEST_RESOURCE_SCENES（`rs_param` 参数）

| 字段名 | 类型 | 示例值 | 说明 |
|--------|------|--------|------|
| `ENV_INFO` | str | `'resource_svr_dev'` | 环境标识 |
| `RESOURCE_INFO` | dict | 见下方 | Resource Server 详情 |
| `RESOURCE_INFO['env']` | str | `'Development'` | 环境类型 |
| `RESOURCE_INFO['is_test']` | bool | `True` | 是否测试环境 |
| `RESOURCE_INFO['lan_ip']` | list | `['29.44.199.147:8080']` | 内网 IP 列表（含端口） |
| `RESOURCE_INFO['lan_name']` | str | `'cloudgame_resource_server_test_internal_network'` | 内网服务名 |
| `RESOURCE_INFO['wan_ip']` | list | `['59.36.157.203:8080']` | 外网 IP 列表（含端口） |
| `RESOURCE_INFO['wan_name']` | str | `'cloudgame_resource_server_test'` | 外网服务名 |

---

## TEST_CGMANAGER_SCENES（`cg_param` 参数）

| 字段名 | 类型 | 示例值 | 说明 |
|--------|------|--------|------|
| `ENV_INFO` | str | `'cgmanager_xf_biyun_865'` | 环境标识 |
| `CGMANAGER_INFO` | dict | 见下方 | CGManager 详情 |
| `CGMANAGER_INFO['instance_id']` | list | `['biyun-foshan-1-...', ...]` | 实例 ID 列表 |
| `CGMANAGER_INFO['lan_ip']` | str | `'172.16.203.41'` | CGManager 内网 IP |
| `CGMANAGER_INFO['wan_ip']` | str | `'183.57.144.39'` | CGManager 外网 IP |

> ⚠️ 注意：`TEST_CGMANAGER_SCENES` 中的 `CGMANAGER_INFO` 只有 `instance_id`、`lan_ip`、`wan_ip` 三个字段，**没有** `port`、`name` 字段（那些在 `TEST_SCENES.CGMANAGER_INFO` 中）。

---

## TEST_WV_VIDEO_SCENES（`wv_param` 参数）

| 字段名 | 类型 | 示例值 | 说明 |
|--------|------|--------|------|
| `ENV_INFO` | str | `'cgmanager_wv_video'` | 环境标识 |
| `WV_VIDEO_INFO` | dict | 见下方 | WV Video 详情 |
| `WV_VIDEO_INFO['cg_ip']` | str | `'30.75.204.158'` | CGManager IP |
| `WV_VIDEO_INFO['proxy_svr']` | str | `'yyb_cloudgame'` | Proxy 服务名 |
| `WV_VIDEO_INFO['proxy_env']` | str | `'Production'` | Proxy 环境 |
| `WV_VIDEO_INFO['proxy_ip']` | list | `['30.8.149.187:24756', ...]` | Proxy IP 列表（含端口） |
| `WV_VIDEO_INFO['pubg_key']` | str | `'pubg_reuse_appversion_key'` | PUBG 复用 key |
| `WV_VIDEO_INFO['lolm_key']` | str | `'lolm_reuse_appversion_key'` | LOLM 复用 key |
| `WV_VIDEO_INFO['cos_appid']` | int | `1258344702` | COS App ID |
| `WV_VIDEO_INFO['cos_bucket']` | str | `'cloudgame-recordtest'` | COS Bucket 名 |
| `WV_VIDEO_INFO['cos_uri']` | str | `'/TEST/wv_video/'` | COS 路径前缀 |
| `WV_VIDEO_INFO['cos_endpoint']` | str | `'cos.ap-nanjing.myqcloud.com'` | COS Endpoint |

---

## 常见访问模式示例

```python
# 模板A：通过 param 访问
proxy_ip = param.PROXY_INFO['ip']
cg_lan_ip = param.CGMANAGER_INFO['lan']
instance_ids = param.XHD_INFO['instance_id']
game_id = param.GAME_ID

# 模板A：直接调用 API 对象
rsp = param.CGAPI.some_method(...)
rsp = param.ProxyAPI.some_method(...)

# 模板B/C：通过模块参数名访问
proxy_ip = proxy_param.PROXY_INFO['ip'][0]   # 注意 ip 是列表
cg_ip = cg_param.CGMANAGER_INFO['wan_ip']
rs_ip = rs_param.RESOURCE_INFO['lan_ip'][0]  # 注意 ip 是列表

# INSTANCE_INFO：通过 instance_id 取单个实例
ins_info = self.INSTANCE_INFO[instance_id]
host_ip = ins_info['host_ip']
ins_ip = ins_info['ip']
```
