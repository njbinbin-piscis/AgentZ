# -*- coding: utf-8 -*-
"""
CloudGame Pytest 测试用例模板

模板选择：
  A = FullLink / XHDModule（XHD 模块或全链路拉流）
  B = 非XHD + 需要实例操作（双参数化）
  C = 非XHD + 纯接口调用（单参数化）

规则：
  - 类名固定 Test，@TestCaseParam 始终第一
  - 环境恢复逻辑统一在 teardown_method 中，用例函数内只设置 flag
  - teardown_method 优先复用 self.xxx 已有变量，没有才重新定义
  - 同目录多个测试文件共用的逻辑封装到 __init__.py
  - 关键字函数优先从 keywords_api.py 获取
"""
from entry import *
from .__init__ import *


class Test():
    @classmethod
    def setup_class(cls):
        pass

    @classmethod
    def teardown_class(cls):
        pass

    def setup_method(self, method):
        pass

    def teardown_method(self, method):
        # 环境恢复：所有清理逻辑集中在此处，通过 flag 触发
        # 优先复用测试函数中已赋值的 self.xxx 变量
        #
        # 模式1: cleanup_flag（通用）
        #   if hasattr(self, "cleanup_flag") and self.cleanup_flag:
        #       self.CGAPI.set_instance_status(...)  # 直接用 self.CGAPI，无需重新初始化
        #
        # 模式2: stop_flag（模板A - XHD 拉流场景）
        #   if hasattr(self, "stop_flag") and self.stop_flag:
        #       self.param.ProxyAPI.stop_game(session_id=self.session_id["sessionid"])
        pass

    # ------------------------------------------------------------------
    # 模板 A: FullLink / XHDModule
    # ------------------------------------------------------------------
    @TestCaseParam
    @pytest.mark.P1
    @pytest.mark.FullLink             # 类型标签（FullLink 或 XHDModule，只能选一个）
    @pytest.mark.Basic                # 场景标签（见 module_config.md 场景标签速查表）
    @pytest.mark.ins_N                # XHD实例标签（N值动态计算，默认N=1）
    @pytest.mark.parametrize(
        "param",
        filter_testcases(TEST_SCENES, ENV_INFO=XF_AOSP13_8550)
    )
    def test_template_a(self, param, step_logger):
        """
        @author: jerryzguo
        @update_person: jerryzguo
        @description: <用例描述>
        """
        self.param = param
        self.stop_flag = False

        step_logger("...")
        # API 调用：self.param.ProxyAPI.<method>()
        # 拉流：thread_player(self.param, INSTANCE_INFO, self.instance_list, cost_time=33)
        # 验证：validate_thread_ret(thread_ret, expected_results)

    # ------------------------------------------------------------------
    # 模板 B: 非XHD + 需要实例操作（双参数化）
    # 各模块替换对照：
    #   proxy       → ProxyModule  / ProxyApi   / proxy_N  / TEST_PROXY_SCENES    / proxy_param
    #   master      → MasterModule / MasterApi  / master_N / TEST_MASTER_SCENES   / master_param
    #   resource_svr→ ResourceSvrModule / ResourceSvrApi / rs_N / TEST_RESOURCE_SCENES / rs_param
    #   cgmanager   → CGManagerModule / CGApi   / cg_N    / TEST_CGMANAGER_SCENES / cg_param
    # ------------------------------------------------------------------
    @TestCaseParam
    @pytest.mark.P1
    @pytest.mark.<类型标签>
    @pytest.mark.<场景标签>
    @pytest.mark.<模块>_N             # 模块实例标签（N值动态计算）
    @pytest.mark.ins_N                # XHD实例标签（N值独立计算，与<模块>_N无需一致）
    @pytest.mark.parametrize("<模块>_param", TEST_<模块大写>_SCENES)
    @pytest.mark.parametrize(
        "param",
        filter_testcases(TEST_SCENES, ENV_INFO=XF_AOSP13_8550)
    )
    def test_template_b(self, <模块>_param, param, step_logger):
        """
        @author: jerryzguo
        @update_person: jerryzguo
        @description: <用例描述>
        """
        self.param = param
        self.<模块>_param = <模块>_param
        self.cleanup_flag = False

        step_logger("获取模块 IP 并初始化 API")
        # IP 提取和 API 初始化见 references/module_config.md

    # ------------------------------------------------------------------
    # 模板 C: 非XHD + 纯接口调用（单参数化，无 param 参数，无 ins_N 标签）
    # 各模块替换对照同模板 B
    # ------------------------------------------------------------------
    @TestCaseParam
    @pytest.mark.P1
    @pytest.mark.<类型标签>
    @pytest.mark.<场景标签>
    @pytest.mark.<模块>_N             # 模块实例标签（N值动态计算）
    @pytest.mark.parametrize("<模块>_param", TEST_<模块大写>_SCENES)
    def test_template_c(self, <模块>_param, step_logger):
        """
        @author: jerryzguo
        @update_person: jerryzguo
        @description: <用例描述>
        """
        self.<模块>_param = <模块>_param
        self.cleanup_flag = False

        step_logger("获取模块 IP 并初始化 API")
        # IP 提取和 API 初始化见 references/module_config.md
        # 关键字函数优先从 keywords_api.py 获取
        # 公共逻辑封装到 __init__.py 中调用
