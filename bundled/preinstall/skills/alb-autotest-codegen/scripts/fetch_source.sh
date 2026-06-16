#!/bin/bash
# ALB/CLB 源码获取脚本
# 用法: bash fetch_source.sh <product>
# 示例: bash fetch_source.sh ALB

PRODUCT=${1:-"ALB"}
PRODUCT_UPPER=$(echo "$PRODUCT" | tr '[:lower:]' '[:upper:]')

# 产品线与仓库映射
declare -A REPO_MAP
REPO_MAP["ALB"]="git@git.woa.com:g_PDC_CGT/alb-api.git"

# 产品线与本地路径映射
declare -A PATH_MAP
PATH_MAP["ALB"]="./alb-api"

REPO_URL=${REPO_MAP[$PRODUCT_UPPER]}
LOCAL_PATH=${PATH_MAP[$PRODUCT_UPPER]}

if [ -z "$REPO_URL" ]; then
    echo "ERROR: 不支持的产品线: $PRODUCT"
    echo "支持的产品线: ${!REPO_MAP[@]}"
    exit 1
fi

echo "=== 获取 $PRODUCT_UPPER 源码 ==="
echo "仓库地址: $REPO_URL"
echo "本地路径: $LOCAL_PATH"

if [ -d "$LOCAL_PATH/.git" ]; then
    echo "仓库已存在，执行 git pull 更新..."
    git -C "$LOCAL_PATH" pull --quiet
    echo "更新完成"
else
    echo "执行 git clone..."
    git clone --branch alb-api_1.0.0 --depth 1 "$REPO_URL" "$LOCAL_PATH" --quiet
    if [ $? -eq 0 ]; then
        echo "clone 成功"
    else
        echo "ERROR: clone 失败，请检查网络和权限"
        exit 1
    fi
fi

# 列出 docs 目录
echo ""
echo "=== API 文档列表 ==="
if [ -d "$LOCAL_PATH/docs" ]; then
    find "$LOCAL_PATH/docs" -name "*.md" -o -name "*.yaml" -o -name "*.json" 2>/dev/null | head -20
else
    echo "未找到 docs 目录，将直接读取源码"
    find "$LOCAL_PATH" -name "*.go" -o -name "*.py" | grep -i "api\|handler\|service" | head -20
fi

echo ""
echo "=== 完成 ==="
