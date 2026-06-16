# 代码模板库

本文档提供各种测试场景的代码模板，可直接复用或参考。

## 一、基础测试类模板

### 1.1 TSDB 测试类模板

```python
# -*- coding: utf-8 -*-
# Copyright (c) 2024 Tencent.com, Inc. All Rights Reserved
"""
Authors: {userid}({userid}@tencent.com)
Date:   {current_date}
"""
import sys
import time

import global_var
from baselib.base_assert import assert_equal, assert_true, assert_not_equal, assert_not_none, assert_in
from case.tsdb.test_tsdb_base import TestTsdbBase
from trpc_client.stub.common_pb2 import ResourceStatus, RaftRole


class Test{ClassName}(TestTsdbBase):
    """
    {测试类描述}
    """

    def __init__(self):
        TestTsdbBase.__init__(self)
        resources_name = "%s-%s" % (self.dp_cluster["name"], "Test{ClassName}")
        self.app_id = resources_name
        self.account_name = resources_name
        self.account_ids = []

    def setUp(self):
        """测试前置条件设置"""
        self.need_recycle_collection = False
        self.clean_env = False
        self.delete_resource = False
        self.blade = False
        
        TestTsdbBase.setUp(self)

    def tearDown(self):
        """测试后置条件清理"""
        global_var.logger.info("-------------<<< Start Test{ClassName} tearDown >>>------------")
        if self.account_id != "":
            self.clean_account_and_resources()
        for account_id in self.account_ids:
            self.account_id = account_id
            self.clean_account_and_resources()
        TestTsdbBase.tearDown(self)

    def test_{case_name}(self):
        """
        test_scene: {测试场景描述}
        test_step:
            1、{步骤1}
            2、{步骤2}
        expect_result:
            1、{预期结果1}
            2、{预期结果2}
        note: {备注}
        level: 0
        author: {userid}
        update_person:
        design_case_uuid: {uuid}
        """
        self.case_name = sys._getframe().f_code.co_name
        global_var.logger.info("start to exec %s" % self.case_name)
        
        # ========== Step 1: 创建账户 ==========
        global_var.logger.info("Step 1: 创建账户")
        account_name = self.account_name + "-1"
        
        # 清理已存在的账户
        existing_id = self.control_plane.sys_admin.get_account_id_by_name_without_app_id(account_name)
        if existing_id != "":
            self.account_id = existing_id
            self.clean_account_and_resources()
        
        # 创建账户
        account_id = self.create_tsdb_resource_and_init_info(
            dba_name=account_name,
            db_name=self.db_name,
            cluster_name=self.dp_cluster['name'],
            alloc_affinity=account_name,
            partition_num_per_coll=1
        )
        assert_not_equal(account_id, "")
        self.account_ids.append(account_id)
        global_var.logger.info("created account_id: %s" % account_id)
        
        # ========== Step 2: 执行测试操作 ==========
        global_var.logger.info("Step 2: 执行测试操作")
        # 实现具体测试逻辑
        
        global_var.logger.info("test case %s finished" % self.case_name)
```

### 1.2 Control Plane 测试类模板

```python
# -*- coding: utf-8 -*-
# Copyright (c) 2024 Tencent.com, Inc. All Rights Reserved
"""
Authors: {userid}({userid}@tencent.com)
Date:   {current_date}
"""
import sys
import time

import global_var
from baselib.base_assert import assert_equal, assert_true, assert_not_equal, assert_not_none
from case.control_plane.test_control_plane_base import TestControlPlaneBase
from trpc_client.stub.common_pb2 import ResourceStatus, APIType


class Test{ClassName}(TestControlPlaneBase):
    """
    {测试类描述}
    """

    def __init__(self):
        super(Test{ClassName}, self).__init__()
        resources_name = "%s-%s" % (self.dp_cluster["name"], "Test{ClassName}")
        self.app_id = resources_name
        self.account_name = resources_name

    def setUp(self):
        """测试前置条件设置"""
        super(Test{ClassName}, self).setUp()
        self.accounts.append(self.account_name)

    def tearDown(self):
        """测试后置条件清理"""
        global_var.logger.info("-------------<<< Start Test{ClassName} tearDown >>>------------")
        self.clean_resources()
        super(Test{ClassName}, self).tearDown()

    def test_{case_name}(self):
        """
        test_scene: {测试场景描述}
        test_step:
            1、{步骤1}
        expect_result:
            1、{预期结果1}
        note: {备注}
        level: 0
        author: {userid}
        update_person:
        design_case_uuid: {uuid}
        """
        self.case_name = sys._getframe().f_code.co_name
        global_var.logger.info("start to exec %s" % self.case_name)
        
        # 测试实现
        
        global_var.logger.info("test case %s finished" % self.case_name)
```

### 1.3 Data Plane 测试类模板

```python
# -*- coding: utf-8 -*-
# Copyright (c) 2024 Tencent.com, Inc. All Rights Reserved
"""
Authors: {userid}({userid}@tencent.com)
Date:   {current_date}
"""
import sys
import time

import global_var
from baselib.base_assert import assert_equal, assert_true, assert_not_equal, assert_not_none
from case.data_plane.test_cluster_case_base import TestClusterCaseBase
from trpc_client.stub.common_pb2 import ConsistencyPolicy, APIType


class Test{ClassName}(TestClusterCaseBase):
    """
    {测试类描述}
    """

    def __init__(self):
        super(Test{ClassName}, self).__init__()
        resources_name = "%s-%s" % (self.dp_cluster["name"], "Test{ClassName}")
        self.app_id = resources_name
        self.dba_name = resources_name
        self.db_name = resources_name
        self.coll_name = resources_name

    def setUp(self):
        """测试前置条件设置"""
        self.delete_resource = False
        super(Test{ClassName}, self).setUp()

    def tearDown(self):
        """测试后置条件清理"""
        global_var.logger.info("-------------<<< Start Test{ClassName} tearDown >>>------------")
        if self.coll_id != "":
            self.yottadb_lib.delete_collection(self.coll_id)
        super(Test{ClassName}, self).tearDown()

    def test_{case_name}(self):
        """
        test_scene: {测试场景描述}
        test_step:
            1、{步骤1}
        expect_result:
            1、{预期结果1}
        note: {备注}
        level: 0
        author: {userid}
        update_person:
        design_case_uuid: {uuid}
        """
        self.case_name = sys._getframe().f_code.co_name
        global_var.logger.info("start to exec %s" % self.case_name)
        
        # Step 1: 创建资源
        partition_id = self.create_resource_and_init_info(
            self.dba_name,
            self.db_name,
            self.coll_name,
            self.dp_cluster['name'],
            storage=50,
            consistency=ConsistencyPolicy.CONSISTENCY_POLICY_STRONG,
            api_type=APIType.API_TYPE_TSSD
        )
        assert_not_equal(partition_id, "")
        
        global_var.logger.info("test case %s finished" % self.case_name)
```

## 二、常用操作模板

### 2.1 账户创建与清理

```python
def create_and_cleanup_account(self, account_suffix):
    """创建账户并注册清理"""
    account_name = self.account_name + account_suffix
    
    # 检查并清理已存在的账户
    existing_id = self.control_plane.sys_admin.get_account_id_by_name_without_app_id(account_name)
    global_var.logger.info("check existing account: %s" % existing_id)
    if existing_id != "":
        self.account_id = existing_id
        self.clean_account_and_resources()
    
    # 创建新账户
    account_id = self.create_tsdb_resource_and_init_info(
        dba_name=account_name,
        db_name=self.db_name,
        cluster_name=self.dp_cluster['name'],
        alloc_affinity=account_name,
        partition_num_per_coll=1
    )
    assert_not_equal(account_id, "")
    
    # 注册到清理列表
    self.account_ids.append(account_id)
    global_var.logger.info("created account: %s -> %s" % (account_name, account_id))
    
    return account_id, account_name
```

### 2.2 SubCluster 操作

```python
def occupy_and_verify_sc(self, account_name, sc_count):
    """占用 SC 并验证"""
    # 占用 SC
    self.occupy_sc(sc_cnt=sc_count, alloc_affinity=account_name)
    
    # 验证占用结果
    sc_list = self.get_sc_ids_by_alloc_affinity(account_name)
    global_var.logger.info("occupied SC count: %d, expected: %d" % (len(sc_list), sc_count))
    assert_equal(len(sc_list), sc_count)
    
    return sc_list


def update_sc_affinity_and_verify(self, sc_ids, new_affinity, expected_status=4):
    """更新 SC 亲和性并验证"""
    global_var.logger.info("updating SC affinity to: %s" % new_affinity)
    
    rsp = self.sys_admin.update_sc_alloc_affinity(
        sub_cluster_ids=sc_ids,
        alloc_affinity=new_affinity
    )
    assert_equal(rsp.err_code, 0)
    
    res = self.control_plane.sys_admin.check_workflow_finished(
        rsp.workflow_id, 
        status=expected_status
    )
    assert_true(res)
    
    global_var.logger.info("SC affinity update workflow finished")
    return rsp.workflow_id
```

### 2.3 工作流操作（Debug 模式）

```python
# 开启 debug 模式
self.yottadb_lib.update_cluster_engine_debug_mode(cls_id=self.cluster_id, debug_mode=True)
global_var.logger.info("debug mode enabled")

# 等待工作流步骤完成
self.wait_workflow_step_finished(workflow_id=workflow_id, expected_step=1)
global_var.logger.info("step 1 finished")

# 回滚步骤
rsp = self.sys_admin.run_step_as_expect(workflow_id=workflow_id, expect=STEP_EXPECT_FAIL)
assert_equal(rsp.err_code, 0)

# 验证工作流回滚完成 (status=6)
res = self.control_plane.sys_admin.check_workflow_finished(workflow_id, status=6)
assert_true(res)

# 重入步骤
rsp = self.sys_admin.run_step_as_expect(workflow_id=workflow_id, expect=STEP_EXPECT_REPEAT)
assert_equal(rsp.err_code, 0)

# 执行下一步
rsp = self.sys_admin.run_step_as_expect(workflow_id=workflow_id, expect=STEP_EXPECT_NEXT)
assert_equal(rsp.err_code, 0)

# 关闭 debug 模式
self.yottadb_lib.update_cluster_engine_debug_mode(cls_id=self.cluster_id, debug_mode=False)
```

## 三、测试执行命令

```bash
# 单个测试用例
nosetests -s --tests /data/zhiyan/yottadb/test/case/{module}/{sub_module}/test_{filename}.py:Test{ClassName}.test_{method_name} > output 2>&1
```
