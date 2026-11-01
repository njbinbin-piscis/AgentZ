# 安全策略

[English](SECURITY.md) | **简体中文**

> 本文是 [SECURITY.md](SECURITY.md) 的中文翻译。两者不一致时以英文版为准。

## 支持的版本

安全修复只针对最新发布版本。报告前请先升级到 [GitHub Releases](https://github.com/njbinbin-piscis/AgentZ/releases) 上的最新版本。

## 报告漏洞

安全问题请**不要**提交公开 issue。

请通过 GitHub 私下报告：打开仓库的 **Security** 标签页，选择 **Report a vulnerability**（[私密漏洞报告](https://github.com/njbinbin-piscis/AgentZ/security/advisories/new)）。请写明：

- 受影响的版本和操作系统；
- 攻击者能做什么，以及需要的前提条件；
- 复现步骤或概念验证；
- 如有修复建议，一并附上。

不要附带真实的 API key、token 或他人的数据。我们力争在 7 天内确认收到报告，并持续同步进展直到解决。修复发布后，会按报告者意愿致谢。

## 范围

范围包括 AgentZ 本身和本仓库中的代码，包括 `vendor/` 下的内置内核和扩展宿主。重点关注：

- 智能体工具执行绕过工作区、审批或策略检查；
- 已保存的 API key 或密钥泄露；
- 远程开发（SSH、容器、WSL）与 agentz-server 控制面；
- 扩展宿主与 webview 隔离。

第三方扩展、MCP 服务器或模型服务商的漏洞，请报告给相应的维护者。
