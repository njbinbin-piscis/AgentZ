# AZ-032 Webview 隔离实现进展

2026-10-06，Codex。用户要求跳过此前GUI验收步骤并继续，故本项持续实现，但真实Tauri Webview 正反验证仍未完成、不能标记done。

当前 `src/extensions/ui/WebviewHost.tsx` 同时给iframe `allow-scripts allow-same-origin`，且将panel API state保存在宿主origin的localStorage，令多个面板共享。当前Tauri CSP `frame-src 'none'` 同时阻断Webview宿主入口。

已改为：Webview默认关闭脚本；仅`enableScripts: true`且宿主页面存在Tauri nonce时开放`sandbox="allow-scripts"`，移除`allow-same-origin`与表单权限；opaque iframe通过精确source、`origin === "null"`、消息封包标记及1Mi字符上限回传；panel state留在每个iframe闭包，不用共享localStorage。生成的srcdoc前置`default-src 'none'`等严格CSP，禁止连接/子frame/表单/对象/base，资源仅限data/blob和带宿主nonce的内联script/style。用HTML扫描器避开注释及script raw-text中形似标签的文本；外部script不带nonce且不能由此CSP放行。生产构建使用Tauri附加到app资源的nonce；devCsp仅在开发环境提供固定nonce以维持调试。Tauri正式构建自动向CSP与应用脚本/样式注入nonce，参照[官方CSP文档](https://v2.tauri.app/security/csp/)。

`npx vitest run src/extensions/ui/webviewSecurity.test.ts`8/8通过；`npm run typecheck`、改动文件ESLint及Tauri配置JSON检查退出0。用例覆盖脚本默认关闭、有效nonce下状态隔离、剥离外链脚本nonce、脚本/样式raw-text保护、doctype、缺少nonce时失败关闭、消息大小界限及不可序列化消息拒绝。当前被隔离的srcdoc基础HTML可渲染。

当时未完成项：`asWebviewUri`作用域映射、外部资源读取、消息边界负例和真实Tauri Webview验证。

## 2026-10-06 追加：每面板资源路由

- 扩展宿主为每个 Webview 生成独立随机 UUID；`asWebviewUri`只接受 file URI，并把绝对路径编码为 `agentz-resource://localhost/{token}/{base64url(path)}`。Windows 的 URI 盘符 `/C:/...` 会先正规化。`cspSource`按 Tauri 平台 origin 规则返回自定义 scheme 或 Windows `.localhost` origin。
- 创建面板时只注册 extension 根目录（默认）或扩展明确提供的 `localResourceRoots`；Rust 规范化注册目录，协议请求按 UUID 找到该面板的根目录，canonicalize 资源文件并拒绝越界、跨面板令牌、非法方法/主机、目录、查询参数和超过32MiB的文件。文件读取放到阻塞线程；面板释放时删除注册。拒绝失败返回404。
- 资源支持受控 CSS、图像、字体和媒体 MIME。内联策略仍禁网络连接/子frame/对象；外部脚本只会由生成器给当前面板令牌 URI 添加页面 nonce，远程/其他面板脚本不会获得 nonce。ZIP 安装也已只读 SKILL.md，不向安装目录直接解压。
- 自动验证：`cargo test --manifest-path src-tauri/Cargo.toml --lib commands::webview_resources::tests:: --locked` 1/1通过（白名单内读取、跨面板拒绝、错误 host 拒绝和越界拒绝）；`npx vitest run src/extensions/ui/webviewSecurity.test.ts extension-host/src/host/webviewResourceUri.test.ts` 13/13通过；根与 extension-host typecheck、受影响文件ESLint、Rust `rustfmt --check`、Tauri JSON解析均通过。
- 真实 Tauri custom-scheme/WebView2/WebKit 请求、CSP 在各平台上的实际执行、运行中页面释放/竞争仍未做桌面验收；用户要求跳过 GUI 步骤，因此只保持 review，不记作通过或 done。当前实现中 SHA/URI 单元测试和模拟协议请求不能替代原验收条款“真实 Tauri Webview 正反测试”。
