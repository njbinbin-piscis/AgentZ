# 当前源码本地Windows候选构建

2026-10-08。AZ018限定本地单Agent候选冻结后转构建，未发布/部署/对外发送。主线程56580 npm run build exit0：tsc与Vite production构建完成，4144模块，Vite耗时5m13s；保留现有大chunk与静态/动态重复导入警告，不假称启动性能已验收。extension-host现有依赖下npm --prefix extension-host run build exit0，host/smoke资源重新生成。

66172 source snapshot与Windows x64 candidate输入离线预检exit0，status verified/candidate_inputs_ready true/release_ready false。该预检指向旧candidate-0.9.7安装器，仅输入可检查，不证明新源码产生该旧产物。新源码snapshot由专用脚本保存，不批量暂存/提交用户工作。

已启动17529：npx tauri build --ci --no-sign --bundles nsis --config target/upgrade-local-build.json -- --offline --locked。临时target overlay仅将beforeBuildCommand置空，因为本轮前端与extension-host已明确构建成功；无需再次npm install或重复前端build，不改tracked发布配置。Windows平台overlay自动合并。旧candidate-0.9.7归档安装器保持独立，新bundle输出完成后另目录保存、hash/预检；当前仍在途，未宣称新安装器完成。

新候选未签名、未安装、未GUI/model/rollback验收，分发授权仍未确认。正式任务24/83与Gate仅M0不变；不因构建成功自动公开发布。

17529最终exit0：当前冻结源码NSIS构建完成；新候选独立保存target/release/candidate-0.9.7-20261008/AgentZ_0.9.7_x64-setup.exe，98,648,395bytes，SHA256 623ed5710ec0ea954c63976aa9c323564bb5ed06ad7098a679387c6d19e3ad11，Authenticode NotSigned。99933实际新产物offline preflight exit0/status verified/candidate_inputs_ready true/release_ready false。旧候选未覆盖。未安装/未GUI模型回滚/未发布；后续AZ023重试改动不包含此安装器。
