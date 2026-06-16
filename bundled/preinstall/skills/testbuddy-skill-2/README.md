安装使用指南:

1. 安装skills技能包
2. 安装mcp配置
{
	"mcpServers": {
		"testbuddy_tools": {
			"url": "http://testbuddy.woa.com/api/tb/v1/cb-plugin/sse",
			"headers": {
				"x-testbuddy-origin": "cb-plugin"
			},
			"timeout": 100000,
			"transportType": "streamable-http",
			"disabled": false
		}
	}
}