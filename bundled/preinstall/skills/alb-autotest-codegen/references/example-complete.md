# 完整测试用例示例（通用接口）

> **权威参考**：非 set 类接口的测试代码必须与此示例风格完全一致。set 类接口请参考 `references/example-set-tests.md`。

本用例展示 create 接口标准流程：获取配置 → 构造参数 → 创建请求 → DB 校验 → get 接口校验 → LD 配置校验 → fixture teardown 清理。

## 完整代码

```python
#!/usr/bin/env python3
# -- coding: utf-8 --

import pytest

__author__ = "aryaliu"


@pytest.fixture
def clean_instance(case_logger, alb_api, alb_config, oss_db):
    """独立的 fixture teardown，使用 yield 模式确保清理"""
    yield
    case_logger.step("【teardown】清理监听器")
    
    app_id = alb_config.get("Account").get("Default").get("AppID")
    product = alb_config.get("Account").get("Default").get("Product")
    
    # 1. 从 DB 查询监听器是否存在（根据 listener_name 查询）
    sql = f"SELECT listener_id, instance_id FROM alb_listener WHERE name='test-https-listener'"
    rows = oss_db.execute_sql(
        command=sql
    )
    if len(rows) > 0:
        for row in rows:
            listener_id = row["listener_id"]
            instance_id = row["instance_id"]
            # 2. 调用 delete 接口删除（禁止直接从 DB 删除）
            params = {
                "product": product,
                "app_id": app_id,
                "instance_id": instance_id,
                "listener_id": listener_id
            }
            delete_response = alb_api.delete_listener(
                data=params
            )
            assert delete_response.get("code") is None or delete_response.get("code") == 0, \
                f"清理数据失败，delete_listener 返回: {delete_response}"
            case_logger.info(f"delete_listener 成功，listener_id={listener_id}")
            
            # 3. 再查 DB 校验数据已被清理
            verify_sql = f"SELECT listener_id FROM alb_listener WHERE listener_id='{listener_id}' AND status != 'deleted'"
            verify_rows = oss_db.execute_sql(
                command=verify_sql
            )
            assert len(verify_rows) == 0, \
                f"调用接口删除监听器成功，但 DB 有残留数据，listener_id={listener_id}"
    else:
        case_logger.info("no listener found, skip cleanup")
    
    case_logger.step("【teardown】清理实例")
    # 同样先查 DB，再调 delete 接口，再校验残留
    sql = f"SELECT instance_id FROM alb_instance WHERE name='instance-auto-test-https'"
    rows = oss_db.execute_sql(
        command=sql
    )
    if len(rows) > 0:
        for row in rows:
            instance_id = row["instance_id"]
            params = {
                "product": product,
                "app_id": app_id,
                "instance_id": instance_id
            }
            delete_response = alb_api.delete_instance(
                data=params
            )
            assert delete_response.get("code") is None or delete_response.get("code") == 0, \
                f"清理实例失败，delete_instance 返回: {delete_response}"
            
            verify_sql = f"SELECT instance_id FROM alb_instance WHERE instance_id='{instance_id}' AND status != 'deleted'"
            verify_rows = oss_db.execute_sql(
                command=verify_sql
            )
            assert len(verify_rows) == 0, \
                f"调用接口删除实例成功，但 DB 有残留数据，instance_id={instance_id}"
    else:
        case_logger.info("no instance found, skip cleanup")


@pytest.mark.P1
@pytest.mark.Auto
@pytest.mark.Controller
@pytest.mark.ALB
@pytest.mark.CaseDescription(
    name="创建HTTPS监听器",
    design_case_uuid="706ae2f9-dc57-4057-b897-c37a88643184",
    tcase_uuid="706ae2f9-dc57-4057-b897-c37a88643184",
    test_scene="【管控】ALB-监听器与规则用例 > 创建监听器 > 正常创建新的监听器 > 创建HTTPS监听器",
    test_step="1、创建实例 create_instance\n2、实例绑定VIP\n3、从数据库读取可用证书\n4、创建目标组\n5、创建HTTPS监听器\n6、验证数据库中的监听器配置\n7、验证Nginx配置",
    expect_result="1、成功\n2、成功\n3、成功\n4、成功\n5、成功\n6、成功\n7、成功",
    note="",
    tapd_url="https://tapd.woa.com/tapd_fe/20426334/story/detail/1020426334129219834",
    create_person="aryaliu",
    update_person="aryaliu",
    version="",
    is_passed=False
)
def test_create_https_listener(
        alb_api,
        case_logger,
        vip_config,
        oss_db,
        alb_config,
        command,
):
    """测试创建HTTPS监听器"""
    case_name = "创建HTTPS监听器"
    case_logger.info(f"开始执行用例: {case_name}")
    
    # 读取VIP配置
    alb_vip_list = vip_config.get("IPv4VPC")
    case_logger.info(f"alb_vip_list: {alb_vip_list}")
    
    vpc_id = alb_vip_list[0]["vpcid"]
    vpc_str = alb_vip_list[0]["vpcid_str"]
    app_id = alb_config.get("Account").get("Default").get("AppID")
    product = alb_config.get("Account").get("Default").get("Product")
    
    # 步骤1: 创建实例 create_instance
    case_logger.step("步骤1: 创建实例 create_instance")
    params = {
        "product": product,
        "app_id": app_id,
        "name": "instance-auto-test-https",
        "vpc_id": vpc_id,
        "vpc_id_str": vpc_str,
        "client_token": "test",
        "address_ip_version": "ipv4",
        "dns_name": f"alb-test-{vpc_id}.tgw.qcloud.com"
    }
    
    response = alb_api.create_instance(
        data=params
    )
    case_logger.info(f"创建实例响应: {response}")
    
    # 验证实例创建成功
    assert "data" in response, "创建实例响应中缺少data字段"
    assert "instance_id" in response["data"], "创建实例响应中缺少instance_id"
    instance_id = response["data"]["instance_id"]
    case_logger.info(f"实例创建成功，instance_id: {instance_id}")
    
    # 验证实例状态（get 接口校验）
    params = {"product": product, "app_id": app_id, "instance_id": instance_id}
    db_config = alb_api.get_instance(
        data=params
    )
    case_logger.info(f"实例配置: {db_config}")
    assert "data" in db_config and "instance_id" in db_config["data"], "实例验证失败"
    
    # 步骤2: 实例绑定VIP
    case_logger.step("步骤2: 实例绑定VIP")
    
    # 2.0 准备工作，查询DB中集群的可用区信息
    params = {"product": product, "max_results": 100}
    set_response = alb_api.get_set(
        data=params
    )
    case_logger.info(f"集群信息: {set_response}")
    
    # 选择一个可用的集群
    choose_en_az_name = []
    choose_az_id = []
    if "set_list" in set_response["data"] and len(set_response["data"]["set_list"]) > 0:
        for info in set_response["data"]["set_list"]:
            if info["status"] != "working":
                continue
            for az in info["az_list"]:
                choose_en_az_name.append(az["en_name"])
                choose_az_id.append(az["az_id"])
            break
    
    assert len(choose_en_az_name) > 0, "没有找到可用的集群"
    case_logger.info(f"选择的可用区: {choose_en_az_name}")
    
    # 2.1 绑定VIP
    az_info = {
        "az_en_name": choose_en_az_name[0],
        "az_id": choose_az_id[0],
        "vip": alb_vip_list[0]["vip"],
        "subnet_id_str": alb_vip_list[0]["subnet_id_str"],
        "subnet_id": alb_vip_list[0]["subnet_id"],
        "subnet_cidr": alb_vip_list[0]["subnet_cidr"]
    }
    
    params = {
        "product": product,
        "app_id": app_id,
        "instance_id": instance_id,
        "vpc_id": vpc_id,
        "vpc_id_str": vpc_str,
        "ops": [az_info],
    }
    
    vip_response = alb_api.add_instance_vips(
        data=params
    )
    case_logger.info(f"绑定VIP响应: {vip_response}")
    
    # 等待VIP绑定任务完成（使用 wait_job_done，不使用 time.sleep）
    if "job_id" in vip_response:
        job_id = vip_response["job_id"]
        param = {"product": product, "app_id": app_id, "job_ids": [job_id]}
        job_result = alb_api.wait_job_done(
            data=param
        )
        if job_result != "success":
            case_logger.error("VIP绑定任务失败")
            assert False, "VIP绑定任务失败"
    
    case_logger.info("VIP绑定成功")
    
    # 步骤3: 从数据库读取可用证书（DB 校验）
    case_logger.step("步骤3: 从数据库读取可用的CA和SVR证书")
    
    # 查询SVR证书（SVR类型）
    sql_svr = f"SELECT cert_id FROM alb_cert WHERE app_id={app_id} AND type='SVR' LIMIT 1"
    svr_cert_rows = oss_db.execute_sql(
        command=sql_svr
    )
    assert svr_cert_rows, f"SQL语句（{sql_svr}）从DB中未查询到可用的SVR证书"
    svr_certificate_id = svr_cert_rows[0]["cert_id"]
    case_logger.info(f"查询到可用的SVR证书: {svr_certificate_id}")
    
    # 步骤4: 创建目标组
    case_logger.step("步骤4: 创建目标组")
    
    health_check = {
        "enable": True,
        "connect_port": 443,
        "protocol": "HTTPS",
        "connect_timeout": 30,
        "codes": ["http_2xx"],
        "http_version": "HTTP1.1",
        "healthy_threshold": 3,
        "unhealthy_threshold": 3,
        "interval": 30,
        "timeout": 10,
        "method": "HEAD",
        "path": "/",
    }
    
    params = {
        "product": product,
        "app_id": app_id,
        "name": "target-group-auto-test-https",
        "client_token": "auto-test-add-target-group-https",
        "vpc_id": vpc_id,
        "vpc_id_str": vpc_str,
        "type": "Instance",
        "scheduler": "wrr",
        "protocol": "HTTPS",
        "keepalive_enable": 1,
        "keepalive_num": 16,
        "health_check": health_check,
    }
    
    response = alb_api.create_target_group(
        data=params
    )
    case_logger.info(f"创建目标组响应: {response}")
    assert "data" in response and "target_group_id" in response["data"], "创建目标组失败"
    target_group_id = response["data"]["target_group_id"]
    case_logger.info(f"目标组创建成功，target_group_id: {target_group_id}")
    
    # 步骤5: 创建HTTPS监听器
    case_logger.step("步骤5: 创建HTTPS监听器")
    
    certificates = [{"cert_id": svr_certificate_id, "type": "SVR"}]
    
    default_rule = {
        "name": "default-rule",
        "priority": 10000,
        "direction": "Request",
        "actions": [{
            "type": "TargetGroup",
            "order": 100,
            "target_group": {
                "target_group_tuples": [{"target_group_id": target_group_id, "weight": 100}],
                "target_group_sticky_session": {"enable": True, "timeout": 10}
            }
        }],
        "conditions": [{"type": "ALL", "values": []}]
    }
    
    params = {
        "product": product,
        "app_id": app_id,
        "instance_id": instance_id,
        "client_token": "test-https-listener",
        "name": "test-https-listener",
        "port": 443,
        "protocol": "HTTPS",
        "gzip_enable": True,
        "http2_enable": True,
        "request_timeout": 60,
        "idle_timeout": 60,
        "certificates": certificates,
        "ca_enable": False,
        "x_forwarded_for_config": {
            "x_forwarded_for_processing_mode": "passthrough",
            "x_forwarded_for_proto_enabled": True
        },
        "rule": default_rule,
    }
    
    listener_response = alb_api.create_listener(
        data=params
    )
    case_logger.info(f"创建监听器响应: {listener_response}")
    
    # 验证监听器创建成功
    assert "data" in listener_response, "创建监听器响应中缺少data字段"
    assert "listener_id" in listener_response["data"], "创建监听器响应中缺少listener_id"
    listener_id = listener_response["data"]["listener_id"]
    case_logger.info(f"HTTPS监听器创建成功，listener_id: {listener_id}")
    
    # 等待监听器创建任务完成
    if "job_id" in listener_response:
        listener_job_id = listener_response["job_id"]
        param = {"product": product, "app_id": app_id, "job_ids": [listener_job_id]}
        job_result = alb_api.wait_job_done(
            data=param
        )
        if job_result != "success":
            case_logger.error("监听器创建任务失败")
            assert False, "监听器创建任务失败"
    
    # 步骤6: 验证数据库中的监听器配置（DB 校验 + get 接口校验）
    case_logger.step("步骤6: 验证数据库中的监听器配置")
    params = {"product": product, "app_id": app_id, "instance_id": instance_id, "listener_id": listener_id}
    
    db_config = alb_api.get_listener(
        data=params
    )
    case_logger.info(f"数据库监听器配置: {db_config}")
    assert "data" in db_config and "listener_id" in db_config["data"], "监听器在数据库中不存在"
    alb_listener_id = db_config["data"]["id"]
    case_logger.info(f"数据库验证成功，alb_listener_id: {alb_listener_id}")
    
    # 步骤7: 验证Nginx配置（LD 配置校验）
    case_logger.step("步骤7: 验证Nginx配置")
    
    # 获取set_id列表
    sql = f"select set_id from alb_instance_vip, alb_instance where alb_instance_vip.alb_instance_id = alb_instance.id and alb_instance.instance_id = '{instance_id}'"
    set_rows = oss_db.execute_sql(
        command=sql
    )
    assert set_rows, f"SQL语句 {sql} 从DB中未查询到VIP信息"
    set_id_list = [row["set_id"] for row in set_rows]
    case_logger.info(f"查询到的set_id列表: {set_id_list}")
    
    # 获取LD端口
    sql = f"select port, set_id FROM alb_listener_ld_port where alb_listener_id='{alb_listener_id}'"
    port_rows = oss_db.execute_sql(
        command=sql
    )
    assert port_rows, f"SQL语句 {sql} 从DB中未查询到LD端口信息"
    
    set_port_map = {row["set_id"]: row["port"] for row in port_rows}
    case_logger.info(f"set_id与端口映射: {set_port_map}")
    
    # 获取LD列表
    param = {"product": product, "set_id_list": set_id_list, "max_results": 100}
    response = alb_api.get_ld(
        data=param
    )
    case_logger.info(f"LD列表响应: {response}")
    assert "data" in response and "set_ld_list" in response["data"], "未查询到LD数据"
    
    ld_ip_list = []
    for set_ld in response["data"]["set_ld_list"]:
        set_id = set_ld["set_id"]
        for ld in set_ld["ld_list"]:
            if ld["status"] != "online":
                continue
            info = {"ip": ld["ld_ip"], "set_id": set_id}
            ld_ip_list.append(info)
    
    assert len(ld_ip_list) > 0, "未找到可用的LD"
    case_logger.info(f"查询到的LD IP列表: {ld_ip_list}")
    
    # 验证Nginx配置文件（LD 配置校验）
    from public_lib.pub_alb.data_plane.alb_nginx import AlbNginx
    file_path = f"{AlbNginx.Path}/conf/"
    cert_path = f"{AlbNginx.Path}/cert/"
    
    for info in ld_ip_list:
        port = set_port_map[info["set_id"]]
        nginx_config_path = f"{file_path}{port}.conf"
        case_logger.info(f"验证LD {info['ip']} 上的Nginx配置文件: {nginx_config_path}")
        
        _, rule_conf, _ = command.executor(
            command=f"cat {nginx_config_path}",
            ip=info["ip"]
        )
        assert rule_conf, f"期望在LD {info['ip']} 上获取到配置文件"
        case_logger.info(f"LD {info['ip']} Nginx配置文件验证完成")
        
        # 验证证书文件是否存在（SVR证书）
        case_logger.info(f"验证LD {info['ip']} 上的SVR证书文件: {cert_path}{svr_certificate_id}.*")
        _, cert_files, _ = command.executor(
            command=f"ls {cert_path}{svr_certificate_id}.* 2>/dev/null | wc -l",
            ip=info["ip"]
        )
        cert_file_count = int(cert_files.strip()) if cert_files.strip() else 0
        assert cert_file_count == 3, f"期望在LD {info['ip']} 上找到3个SVR证书文件，实际找到{cert_file_count}个"
        case_logger.info(f"LD {info['ip']} SVR证书文件验证完成，找到{cert_file_count}个文件")
    
    case_logger.info(f"用例 {case_name} 执行完成")
```

> **详细规范**：参见 `references/standards.md`
