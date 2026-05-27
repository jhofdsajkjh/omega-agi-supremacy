# 🚀 OMEGA AGI Supremacy

> 超越 OpenHuman 和 Hermes-Agent 的超级 AGI 系统

## 架构概览

```
Layer 4 - Evolution    │ 自进化引擎 (Φ_APEX*∞)
Layer 3 - Engineering   │ 代码生成/测试/PR/质量门禁
Layer 2 - Swarm         │ 多Agent协作/共识/CRDT
Layer 1 - Runtime       │ Actor系统/WASM沙箱/图执行器
Layer 0 - HyperCore     │ 内存/调度/安全/会话 (Rust)
```

## 竞争优势

| 特性 | OpenHuman | Hermes-Agent | OMEGA AGI |
|------|-----------|--------------|-----------|
| 安全漏洞 | 4高危 | 14严重 | **0漏洞** |
| 架构层数 | 3层 | 2层 | **5层** |
| 多Agent协作 | ❌ | ❌ | **✅ Swarm** |
| 自进化 | ❌ | ❌ | **✅ 24/7** |
| 主动漏洞预测 | ❌ | ❌ | **✅ Super AGI** |

## 🚀 快速开始

### 一键安装 (推荐)

```bash
# 克隆项目
git clone https://github.com/hernandez42/omega-agi-supremacy.git
cd omega-agi-supremacy

# 一键安装向导
bash install.sh
```

向导会自动检测环境、安装依赖、引导配置。完成后访问 **http://localhost:5000**


### 详细步骤

详见 [QUICKSTART.md](QUICKSTART.md)：

```bash
# 1. 克隆
git clone https://github.com/hernandez42/omega-agi-supremacy.git
cd omega-agi-supremacy

# 2. 自动安装
bash install.sh

# 3. 启动服务
bash launcher.sh

# 4. 访问 Web UI
open http://localhost:5000
```


---

## 🛠️ 安装向导

OMEGA AGI 提供三套安装/配置工具，满足不同场景：

| 工具 | 命令 | 适用场景 |
|------|------|----------|
| 一键安装 | `bash install.sh` | 首次安装，全程引导 |
| 交互式向导 | `python3 setup_wizard.py` | 详细配置 LLM 和服务 |
| 服务启动器 | `bash launcher.sh` | 启动/停止/查看状态 |

### install.sh - 一键安装脚本

自动检测环境 + 安装依赖 + 引导配置：


```bash
# 交互式安装
bash install.sh

# 静默安装 (使用环境变量)
OMEGA_API_KEY=your-key OMEGA_MODEL_NAME=gpt-4o bash install.sh --silent
```

### setup_wizard.py - 交互式配置向导

带 Rich UI 的交互式向导，支持：

- LLM 提供商选择 (Groq/OpenAI/Anthropic/DeepSeek/Ollama)
- API Key 配置与验证
- 启动模式选择
- 配置保存与健康检查

```bash
python3 setup_wizard.py
```

### launcher.sh - 服务启动器

智能启动器，支持多种操作模式：


```bash
bash launcher.sh              # 启动所有服务
bash launcher.sh --status     # 查看服务状态
bash launcher.sh --stop       # 停止所有服务
bash launcher.sh --restart   # 重启所有服务
bash launcher.sh --help       # 显示帮助
```


### Web UI 向导

首次访问 http://localhost:5000/wizard 会自动显示 4 步引导流程：

1. **欢迎页** - 介绍配置步骤
2. **LLM 配置** - 选择提供商、输入 API Key、选择模型
3. **服务配置** - 选择启动模式、配置端口
4. **确认启动** - 摘要确认、一键启动

---


## 📁 项目结构


```
omega-agi-supremacy/
├── install.sh              # 一键安装脚本 ⭐
├── setup_wizard.py         # 交互式配置向导 ⭐
├── launcher.sh            # 服务启动器 ⭐
├── .env                    # 配置文件 (自动生成)
├── QUICKSTART.md           # 5分钟快速开始指南 ⭐
├── omega-agi/              # 核心 Agent 代码
│   └── web_ui/             # Web UI
│       ├── app.py          # Flask 应用
│       └── templates/
│           ├── index.html   # 控制台仪表盘
│           ├── config.html  # LLM 配置管理
│           └── wizard.html  # Web UI 向导 ⭐
├── scripts/                # 辅助脚本
│   ├── health_check.sh     # 健康检查
│   └── auto_recovery.sh    # 自动恢复
└── deploy.sh               # Docker 部署脚本
```


---


## 🔧 命令参考

| 命令 | 说明 |
|------|------|
| `bash install.sh` | 一键安装/重新安装 |
| `python3 setup_wizard.py` | 交互式配置向导 |
| `bash launcher.sh` | 启动所有服务 |
| `bash launcher.sh --status` | 查看服务状态 |
| `bash launcher.sh --stop` | 停止所有服务 |
| `bash launcher.sh --restart` | 重启所有服务 |
| `bash scripts/health_check.sh` | 健康检查 |

---


## ⚙️ 配置文件

主配置文件 `.env` (自动生成)：

```bash
# LLM Provider (1=OpenAI, 2=Anthropic, 3=Groq, 4=DeepSeek, 5=Ollama)
OMEGA_LLM_PROVIDER=3
OMEGA_API_URL=https://api.groq.com/openai/v1/chat/completions
OMEGA_API_KEY=your-api-key
OMEGA_MODEL_NAME=llama-3.3-70b-versatile

# 服务配置
OMEGA_WEB_PORT=5000
OMEGA_WEB_HOST=0.0.0.0
OMEGA_STARTUP_MODE=development

# 可选
GITHUB_TOKEN=ghp_...
OMEGA_LOG_LEVEL=INFO
OMEGA_DEBUG=false
```

---

## ❓ 常见问题

### Q: Python 未安装
```bash
# Ubuntu/Debian
sudo apt update && sudo apt install python3 python3-pip
# macOS
brew install python3
```

### Q: 端口 5000 被占用
```bash
# 修改端口
echo "OMEGA_WEB_PORT=8080" >> .env
# 或查看占用进程
lsof -i :5000
```

### Q: API Key 无效
```bash
# 重新配置向导
python3 setup_wizard.py
```

---

## 快速部署

```bash
# 1. 克隆仓库
git clone https://github.com/hernandez42/omega-agi-supremacy.git
cd omega-agi-supremacy

# 2. 配置环境变量
cp .env.example .env
# 编辑 .env 填入你的API密钥

# 3. 一键部署
chmod +x deploy.sh && ./deploy.sh

# 4. 启动系统
docker-compose up -d --build

# 5. 查看日志
docker logs -f omega_agi_core
```

## 测试

```bash
# Rust 测试 (Layer 0-1)
cd hypercore && cargo test
cd runtime && cargo test

# Python 测试 (Layer 4)
cd omega_pipeline && python3 -m pytest

# 安全扫描
python3 omega_pipeline/super_agi_predictor.py scan .
```

## 技术栈

- **核心**: Rust 1.95 + Python 3.11
- **容器**: Docker + Docker Compose
- **LLM**: GPT (OpenAI) + Claude (Anthropic)
- **安全**: Capability-based + 零信任
- **协作**: CRDT + Raft 共识

## 适配器层 (Adapters)

OMEGA AGI 提供与多个 Agent 系统的兼容性适配层：

| 适配器 | 消息发送 | Skill加载 | 工作流执行 | Agent协议 |
|--------|----------|-----------|------------|------------|
| OpenClaw | ✅ | ✅ | ❌ | ✅ |
| Hermes | ✅ | ❌ | ✅ | ✅ |
| OpenHuman | ✅ | ❌ | ✅ | ✅ |


### OpenClaw 适配器
- 飞书消息协议兼容
- Skill 动态加载
- Interactive Card 支持

### Hermes 适配器
- Hermes 工作流定义支持
- 任务状态跟踪
- 重试配置

### OpenHuman 适配器
- Agent 请求/响应模式
- 工作流图执行
- 上下文变量传递

### 使用示例

```rust
use omega_adapters::{AdapterManager, OpenClawAdapter, HermesAdapter};


// 创建适配器管理器
let manager = AdapterManager::new();


// 切换活动适配器
manager.set_active("hermes").unwrap();

// 获取适配器信息
let info = manager.get_active_info();
```

## License

MIT
