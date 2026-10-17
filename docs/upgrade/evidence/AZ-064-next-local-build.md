# 下一Windows候选构建准备

2026-10-09。冻结范围为已验证AZ023重试、AZ035统一本地详情、AZ021限定取消完成屏障及原有本地候选；完整费用执行、团队/remote全资源确认后移。

source snapshot：AZ-064-next-source-snapshot.json。工作区有未提交源码，snapshot非Git HEAD重现保证；保留用户修改，不批量stage/reset。实际扩展宿主npm build exit0。前端53861 tsc/Vite生产build仍运行，完成前不复用旧dist启动新安装器。新NSIS尚未启动、不称完成。

取消宿主3、kernelregistry4、invocation2实际通过，前端详情11/tsc/lint通过，宿主详情4、plan只读16通过。真实桌面、模型、安装回滚、平台与签名未验；不自动发布。

53861实际frontend tsc/Vite生产build exit0，4146模块，Vite5分37秒；动态/静态import与大chunk警告保留。扩展宿主build已exit0。资源hash已冻结next-build-inputs，启动一次offline locked未签名NSIS；无安装/发布。

66034最终exit0：NSIS完成，新包独立保存candidate-0.9.7-20261009，98,687,336bytes，SHA256 d0844c488dbfaedced4aaa1019774293807903c2317f67f9438a8712d1a3395e，Authenticode NotSigned。314份冻结源码再核无变化，旧20261008候选保留。首次artifact preflight用了相对路径被拒exit2，改绝对路径补验，未假称首次通过。未安装/GUI模型/回滚/发布。

4592绝对路径artifact offline preflight最终exit0，status verified、candidate_inputs_ready true、release_ready false。离线摘要校验不是真实安装/执行或正式发行批准。
