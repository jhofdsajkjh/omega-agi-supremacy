#!/bin/bash
# ============================================================
#  OMEGA AGI 一键部署脚本
#  服务器: 新加坡 43.156.126.35
#  用户: ubuntu
#  用法: chmod +x deploy.sh && ./deploy.sh
# ============================================================

set -e

echo "================================================"
echo "  🚀 OMEGA AGI 一键部署系统"
echo "  超越 OpenHuman & Hermes-Agent"
echo "================================================"

# ---- 1. 系统更新 ----
echo ""
echo "[1/7] 更新系统..."
sudo apt-get update -qq

# ---- 2. 安装 Docker ----
echo ""
echo "[2/7] 安装 Docker..."
if ! command -v docker &> /dev/null; then
    curl -fsSL https://get.docker.com | sudo sh
    sudo usermod -aG docker ubuntu
    echo "Docker 安装完成"
else
    echo "Docker 已安装: $(docker --version)"
fi

# ---- 3. 安装 Docker Compose ----
echo ""
echo "[3/7] 安装 Docker Compose..."
if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
    sudo apt-get install -y docker-compose-plugin 2>/dev/null || \
    sudo curl -L "https://github.com/docker/compose/releases/latest/download/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose && \
    sudo chmod +x /usr/local/bin/docker-compose
    echo "Docker Compose 安装完成"
else
    echo "Docker Compose 已安装"
fi

# ---- 4. 安装基础依赖 ----
echo ""
echo "[4/7] 安装基础依赖..."
sudo apt-get install -y -qq python3 python3-pip git curl wget jq > /dev/null 2>&1
pip3 install --break-system-packages -q requests aiohttp 2>/dev/null || true

# ---- 5. 创建项目目录 ----
echo ""
echo "[5/7] 创建项目目录..."
PROJECT_DIR="$HOME/omega-agi"
sudo mkdir -p "$PROJECT_DIR"/{omega-agi/{hypercore,runtime},omega_pipeline,logs,data}
sudo chown -R ubuntu:ubuntu "$PROJECT_DIR"

# ---- 6. 配置环境变量 ----
echo ""
echo "[6/7] 配置环境变量..."
cat > "$PROJECT_DIR/.env" << 'ENVEOF'
# OMEGA AGI 环境配置
# LLM API 配置
OPENAI_API_BASE=https://api-t2-sg.freemodel.dev/v1
OPENAI_API_KEY=fe_oa_71edab031ee8e0255a502c782e06dec3bd8d29bfa0d83a92
ANTHROPIC_API_BASE=https://api-cc.freemodel.dev
ANTHROPIC_API_KEY=sk-ant-omega-agi

# 系统配置
AUTONOMOUS_MODE=true
PHASE=supremacy
LOG_LEVEL=info
TZ=Asia/Shanghai

# 安全配置
SECURITY_LEVEL=high
SANDBOX_ENABLED=true
CAPABILITY_SECURITY=true

# 进化配置
EVOLUTION_ENABLED=true
EVOLUTION_INTERVAL=3600
S_PLUS_TARGET=0.95
ENVEOF

echo "环境变量已配置"

# ---- 7. 创建 Docker Compose ----
echo ""
echo "[7/7] 创建 Docker Compose 配置..."

cat > "$PROJECT_DIR/docker-compose.yml" << 'COMPOSEEOF'
version: '3.8'

services:
  omega-core:
    build:
      context: .
      dockerfile: Dockerfile
    container_name: omega_agi_core
    restart: unless-stopped
    env_file:
      - .env
    volumes:
      - ./omega-agi:/app/omega-agi
      - ./omega_pipeline:/app/omega_pipeline
      - ./logs:/app/logs
      - ./data:/app/data
      - /var/run/docker.sock:/var/run/docker.sock
    ports:
      - "8080:8080"
    networks:
      - omega_network
    healthcheck:
      test: ["CMD", "python3", "-c", "import urllib.request; urllib.request.urlopen('http://localhost:8080/health')"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 30s

  self-healing:
    build:
      context: .
      dockerfile: Dockerfile.heal
    container_name: omega_self_healing
    restart: unless-stopped
    env_file:
      - .env
    volumes:
      - ./logs:/app/logs
      - /var/run/docker.sock:/var/run/docker.sock
    networks:
      - omega_network
    depends_on:
      - omega-core

networks:
  omega_network:
    driver: bridge
COMPOSEEOF

echo "Docker Compose 配置完成"

# ---- 创建 Dockerfile ----
cat > "$PROJECT_DIR/Dockerfile" << 'DFEOF'
FROM python:3.11-slim

LABEL maintainer="OMEGA AGI <omega@agi.system>"
LABEL description="OMEGA AGI - 超越OpenHuman和Hermes-Agent的超级AGI系统"

# 安装系统依赖
RUN apt-get update && apt-get install -y --no-install-recommends \
    git curl wget jq \
    && rm -rf /var/lib/apt/lists/*

# 安装Python依赖
RUN pip3 install --no-cache-dir --break-system-packages \
    requests aiohttp fastapi uvicorn \
    pydantic python-dotenv \
    2>/dev/null || pip install --no-cache-dir \
    requests aiohttp fastapi uvicorn \
    pydantic python-dotenv

WORKDIR /app

# 复制代码
COPY omega_pipeline/ /app/omega_pipeline/
COPY omega-agi/ /app/omega-agi/

# 设置权限
RUN chmod +x /app/omega_pipeline/*.py 2>/dev/null || true

# 健康检查端点
EXPOSE 8080

# 启动命令
CMD ["python3", "-u", "/app/omega_pipeline/supremacy_autopilot.py"]
DFEOF

# 自愈容器 Dockerfile
cat > "$PROJECT_DIR/Dockerfile.heal" << 'DHEALEOF'
FROM python:3.11-slim

RUN pip3 install --no-cache-dir --break-system-packages requests 2>/dev/null || \
    pip install --no-cache-dir requests

WORKDIR /app
COPY omega_pipeline/self_healing.py /app/

CMD ["python3", "-u", "/app/self_healing.py"]
DHEALEOF

echo ""
echo "================================================"
echo "  ✅ 部署准备完成！"
echo "================================================"
echo ""
echo "  项目目录: $PROJECT_DIR"
echo "  下一步操作:"
echo ""
echo "  1. 上传代码到服务器:"
echo "     scp -r omega-agi/ omega_pipeline/ ubuntu@43.156.126.35:~/omega-agi/"
echo ""
echo "  2. 在服务器上启动:"
echo "     cd ~/omega-agi && docker-compose up -d --build"
echo ""
echo "  3. 查看日志:"
echo "     docker logs -f omega_agi_core"
echo ""
echo "  4. 检查状态:"
echo "     curl http://localhost:8080/health"
echo ""
echo "================================================"
