# AZ-021 owned worker completion 核心候选

2026-10-09。仅common kernel agent/tool_completion.rs、agent/tool.rs accessor、agent/mod.rs导出，以及当前vendor已沙箱化shell/code_run两个blockingworker接线。未改host/台账/Cargo，未运行Cargo、真实模型或进程验收。独占单文件rustfmt完成。

## 契约

`piscis_kernel::agent::tool_completion::for_cancel(&Arc<AtomicBool>)->Arc<OwnedCompletionRegistry>`，host必须在启动Attempt前capture并retain；每Attempt使用fresh cancel Arc。内部weak索引按Arc地址，registry强持原Arc防地址复用；same capturedcancel共享，新flag隔离，不按model/session名猜归属。无需新增ToolContext字段，保持约20个struct literal兼容；ctx.owned_completion()提供accessor。

register在spawn前取得OwnedReceipt，cancel或sealed后拒绝新ownedworker。receipt必须move进blockingclosure，真实Job/后代/pipe/ACL cleanup返回后complete(Result<(),String>)。Drop awaited toolfuture不会drop worker里的receipt；panic/未complete Drop记录Failed而非Confirmed。仅保留failure计数，不存原始错误/凭据。

drain(timeout)先seal，再有界等待，返回Complete / Pending{active} / Failed{active,failures}。drop drainfuture不释放worker、不自动重试；仍活跃失败到timeout才返回带active失败，不伪造已收割。host必须保留attempt/gate/slot直到真实结论并按未确认分类Interrupted，当前host消费由另一代理/主线程独立接线。本注册表本身不授OS能力，也不确认未登记工具。

## 实际源码范围与源仓差异

common三目标source/vendor字节相同：

- tool_completion.rs `410cf09ca388a927c7394aaf2192ffe7bf5df3c43dd51eec4ae95affeddd8622`
- tool.rs `7a5ad41c1f1c47c31aefee00d00da9aceb95620b60b52da8cfee1723d9c6d4fa`
- mod.rs `245e7169f40e7e77eade00d119f277e4532cbc275df77b246881856e6009204e`

当前source shell/code_run仍旧unsandboxed，而vendor已有AZ034 AppContainer闭包。没有覆盖/偷偷同步源仓这套大范围既有差异，已报告主线程。只给vendor沙箱worker加completion overlay：shell `c6e01b22b39bffd1fed6269760bb0a8ca20a8125aadcae2464ddb2725f21f552`，code_run `c1f8cfab14a367cf09e7c291768aee7f5df25b6346f62a4377ab463c73414178`。这两个工具不声称source/vendor整文件相同；common源码仓可编译契约与产品vendor工具消费分层。

## 补丁验证实际执行

common `AZ-021-owned-completion-v0.8.68.patch` SHA256 `2bdd34accf117febe680477e4c4ebf1512654c8ef31bfeaa6d3a68de64523dda`；vendor overlay `AZ-021-vendor-sandbox-completion-v0.8.68.patch` `ec0ae6ddacaf48225e3d04c6f90c0846dc4a7a49cef43cbde4ee9bb902815566`。

Python隔离目标fixture实际apply --check/apply、finalbytes、reverse --check/reverse、baselinebytes及newmodule移除均通过exit0，目录.cache/az021-patch-vksus430与az021-patch-fww3k7a0。common基线取source HEAD的两个未dirty目标；overlay基线是此次编辑前vendor工具局部快照（LF规范化），不宣称完整固定vendor重建。首次验证因CRLF/git autocrlf造成bytes不等失败，未记通过；在独占目标LF规范化和隔离仓core.autocrlf=false后重跑通过，未改全局/源仓Git配置。

## 四个mock fixture源码（未实跑）

真实spawn_blocking receipt在await JoinHandle丢弃后仍Pending直到受控cleanup放行；drop/error receipt不能Confirm；相同cancel共享与另Attempt隔离；cancel和seal拒新worker。测试不用真实模型/外部网络，不等于Windows真实收割验收。主线程统一目标Cargo/strictClippy与host接线验证。

## 未覆盖

未登记tools/remote/browser/UI、source旧unsandboxed工具、跨进程boot归属、实际Windows父子进程/ACL恢复、全取消路径和GUI。host未接前Cancelled语义漏洞尚未完全修复；本core候选与补丁通过不能让AZ021 done或正式Gate提升。
