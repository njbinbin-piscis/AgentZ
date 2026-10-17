# Windows x64 RC Node runtime 字节一致性

日期：2026-10-07。只读本地摘要与官方HTTPS清单核对；未下载或安装runtime，未执行node.exe，未签名/发布，未改变resource-integrity.json或任何approved preinstall记录。

## 本轮实测

本地：`C:/Projects/AgentZ/src-tauri/resources/node/win32-x64/node.exe`，80,511,640字节。extension-host/scripts/fetch-node.mjs声明固定Node版本22.11.0，Windows下载目标为win-x64/node.exe。

使用浏览工具读取[Node.js官方v22.11.0 SHA-256清单](https://nodejs.org/download/release/v22.11.0/SHASUMS256.txt)，并用PowerShell Invoke-WebRequest读取同一官方HTTPS文本，HTTP200。官方发布页也提供[Node.js 22.11.0发布记录](https://nodejs.org/en/blog/release/v22.11.0)。/dist/v22.11.0/清单在浏览工具中不可访问，改用同域官方/download/release路径成功；未转向第三方镜像。

捕获时间：2026-10-07T05:59:21.0945307Z（上海时间13:59）。HTTP响应Last-Modified为Tue, 29 Oct 2024 14:03:19 GMT，ETag为W/"50fddcd0e227c06348d3b323f902a46c"。清单解码文本按UTF-8编码SHA-256：`dddf2b497e71a131348716f78662c9fd51b9a30f808d5c5b81e7f3c7824a83fb`；该摘要描述本次收到的清单文本，不是项目approved摘要。

| 对象 | SHA-256 |
| --- | --- |
| 官方清单win-x64/node.exe条目 | 7447c4ece014aa41fb2ff866c993c708e5a8213a00913cc2ac5049ea3ffc230d |
| 本地node.exe第一次Get-FileHash | 7447c4ece014aa41fb2ff866c993c708e5a8213a00913cc2ac5049ea3ffc230d |
| HTTP读取后再次Get-FileHash | 7447c4ece014aa41fb2ff866c993c708e5a8213a00913cc2ac5049ea3ffc230d |

比较matches=true，命令会话88176 exit0。结论限于：**当前捆绑Windows x64 runtime字节与Node.js官方HTTPS发布清单相应摘要一致，可作为Windows x64候选构建的字节来源核对证据。**

## 信任与放行边界

- 清单来自nodejs.org HTTPS并通过当前系统的TLS信任链取得；未独立验证SHASUMS签名、发布者PGP身份或Node二进制Authenticode身份。官方发布页展示PGP签名块，不等于本轮已做密码学验签。
- 清单与字节摘要一致不代表上游签名有效、项目发行身份已批准、preinstall完整资源摘要已批准、最终安装包已签名或发行门禁通过。
- 未制造approved preinstall SHA、签名证书身份或资源批准记录；resource-integrity.json未更改。
- 此证据不检查其他上游资源、其他架构/平台、安装/启动/卸载或回滚；也不是完整上游供应链资源审计。上述项目记录下一轮，不阻止当前候选的其他已授权工程，但不放行发布。
- 本地文件若随后改变，需重新读取hash核对，不复用本次结果替代新产物检查。
