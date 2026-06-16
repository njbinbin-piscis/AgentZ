# 源码获取与解析指南

## 一、产品线识别规则

从用户输入中提取产品关键词，匹配规则（大小写不敏感）：

| 用户输入示例 | 识别结果 |
|-------------|----------|
| "ALB 的 AddSet 接口" | ALB |
| "alb 集群创建" | ALB |

## 二、源码获取流程

### 2.1 执行 git clone

```bash
# ALB
git clone --branch alb-api_1.0.0 --depth 1 git@git.woa.com:g_PDC_CGT/alb-api.git ./alb-api/ 2>/dev/null || \
  (cd ./alb-api && git pull)
```

> 若 clone 失败（网络问题/权限问题），提示用户手动 clone 并告知路径。

### 2.2 文档优先策略

按以下优先级查找接口定义：

1. **`docs/` 目录**：优先读取 Markdown 或 OpenAPI/Swagger 文档
   ```bash
   find ./alb-api/docs -name "*.md" -o -name "*.yaml" -o -name "*.json" | head -20
   ```

2. **`api/` 或 `proto/` 目录**：读取接口定义文件
   ```bash
   find ./alb-api -name "*.proto" -o -name "*_api.go" -o -name "*_api.py" | head -20
   ```

3. **源码目录**：直接读取实现代码
   ```bash
   find ./alb-api/logic -name "*.go" -o -name "*.go" | grep -i "set\|listener\|rule" | head -20
   ```

### 2.3 接口信息提取

从文档/源码中提取以下信息，下面是一个参考信息：

```
接口名称: AddSet
请求路径: /api/v1/set/add
请求方法: POST
请求参数:
  - product: string, 必填, 产品标识
  - set_name: string, 必填, 集群名称
  - region: string, 必填, 地域
  - az_list: array, 必填, 可用区列表
  - remark: string, 可选, 备注
响应字段:
  - code: int, 错误码（0=成功）
  - message: string, 错误信息
  - data.set_id: int, 创建的集群ID
DB 表: alb_set
关联接口: GetSet（查询集群详情）
```

## 三、本地已有封装优先

在生成测试代码前，**优先检查** `public_lib/pub_alb/control_plane/alb_api.py` 中是否已有对应接口的封装：

```bash
grep -n "def add_set\|def get_set\|def delete_set" public_lib/pub_alb/control_plane/alb_api.py
```

若已有封装，直接使用 `alb_api.add_set(data=...)` 调用，无需重新封装。

若无封装，使用 参考文件下，其他接口的实现形式，先实现对应的接口，然后再调用。

## 四、DB 表结构获取

若需要 DB 校验，从以下途径获取表结构：

1. 查看 `repo/db_repo/entity/` 下的数据库设计文档


示例：
```go
-- albAz表结构
type AlbAz struct {
	Id         int64     `gorm:"column:id" json:"id"`
	AzId       int64     `gorm:"column:az_id" json:"az_id"`
	EnName     string    `gorm:"column:en_name" json:"en_name"`
	ZhName     string    `gorm:"column:zh_name" json:"zh_name"`
	CreateTime time.Time `gorm:"column:create_time" json:"create_time"`
	UpdateTime time.Time `gorm:"column:update_time" json:"update_time"`
}

```
