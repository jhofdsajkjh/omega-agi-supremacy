# 🚀 OMEGA AGI Supremacy - 5分钟快速开始

> 让任何人都能像安装 OpenClaw 一样简单配置和启动 OMEGA AGI

---

## TL;DR 一键安装

```bash
git clone https://github.com/your-repo/omega-agi-supremacy.git
cd omega-agi-supremacy
bash install.sh
```

然后访问 **http://localhost:5000** 即可。

---

## 📋 准备工作

- Python 3.8+
- Git
- LLM API Key (可选，推荐 Groq 免费额度)

---

## 步骤 1：克隆项目

```bash
git clone https://github.com/your-repo/omega-agi-supremacy.git
cd omega-agi-supremacy
```

## 步骤 2：自动安装

运行一键安装向导：

```bash
bash install.sh
```

向导会自动：
- ✅ 检测系统环境 (Python、磁盘空间、网络)
- ✅ 安装项目依赖
- ✅ 引导配置 LLM API
- ✅ 保存配置文件

**输出示例：**
```
🚀 欢迎使用 OMEGA AGI Supremacy
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

[1/6] 系统环境检测 ... ✅
[2/6] 安装依赖 ... ✅
[3/6] LLM API 配置 ... ✅
[4/6] GitHub Token (可选) ... ✅
[5/6] 启动配置 ... ✅
[6/6] 启动服务 ... ✅

🎉 安装完成！访问 http://localhost:5000
```

## 步骤 3：交互式配置 (可选)

如果 `install.sh` 跳过了一些配置，运行向导：

```bash
python3 setup_wizard.py
```

这是一个交互式图形界面，会一步步引导你配置：
- 选择 LLM 提供商 (Groq/OpenAI/Anthropic/DeepSeek/Ollama)
- 输入 API Key
- 选择启动模式
- 验证配置

## 步骤 4：启动服务

```bash
# 启动所有服务
bash launcher.sh

# 查看状态
bash launcher.sh --status

# 停止服务
bash launcher.sh --stop
```

## 步骤 5：访问 Web UI

打开浏览器访问：

```
http://localhost:5000        # 本机访问
http://<你的IP>:5000         # 局域网访问
```

首次访问会自动显示 **Web 向导**，引导你完成最后配置。

---

## 🎯 快速配置推荐

### 推荐：Groq (免费高速)

```bash
# 在向导中选择 [3] Groq
# API Key: https://console.groq.com/api-keys (免费注册)
# 模型: llama-3.3-70b-versatile (推荐)
```

### 备选：OpenAI (需要付费)

```bash
# 在向导中选择 [1] OpenAI
# API Key: https://platform.openai.com/api-keys
# 模型: gpt-4o (推荐)
```

---

## 📁 项目结构

```
omega-agi-supremacy/
├── install.sh              # 一键安装脚本
├── setup_wizard.py         # 交互式配置向导
├── launcher.sh            # 服务启动器
├── .env                    # 配置文件 (自动生成)
├── omega-agi/              # 核心 Agent 代码
│   └── web_ui/             # Web UI
│       ├── app.py          # Flask 应用
│       └── templates/
│           ├── index.html  # 控制台
│           ├── config.html # LLM 配置页
│           └── wizard.html # 首次配置向导 ⭐
├── scripts/                # 辅助脚本
│   ├── health_check.sh     # 健康检查
│   └── auto_recovery.sh    # 自动恢复
└── QUICKSTART.md           # 本文档
```

---

## ⚡ 常用命令

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

## 🔧 配置文件说明

配置文件位于 `.env`：

```bash
# LLM 配置
OMEGA_LLM_PROVIDER=3        # 1=OpenAI, 2=Anthropic, 3=Groq, 4=DeepSeek, 5=Ollama
OMEGA_API_URL=https://api.groq.com/openai/v1/chat/completions
OMEGA_API_KEY=your-api-key
OMEGA_MODEL_NAME=llama-3.3-70b-versatile

# 服务配置
OMEGA_WEB_PORT=5000
OMEGA_WEB_HOST=0.0.0.0
OMEGA_STARTUP_MODE=development

# 可选
GITHUB_TOKEN=ghp_...        # GitHub Token
OMEGA_LOG_LEVEL=INFO
OMEGA_DEBUG=false
```

---

## 🆘 常见问题

### Q: 提示 "Python 未安装"
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

# 或查找占用进程
lsof -i :5000
```

### Q: API Key 无效
```bash
# 重新配置
python3 setup_wizard.py

# 或手动编辑
nano .env
```

### Q: Groq 免费额度用完
- 切换到其他提供商
- 或访问 https://console.groq.com 升级付费计划

### Q: Web UI 无法访问
```bash
# 检查服务状态
bash launcher.sh --status

# 查看日志
tail -f logs/web_ui.log
```

---

## 🌟 下一步

- 📖 阅读 [README.md](README.md) 了解完整功能
- 🔧 配置 LLM: 访问 http://localhost:5000/config
- 📊 查看状态: 访问 http://localhost:5000/status
- 🔍 自诊断: 访问 http://localhost:5000/diagnostics

---

_OMEGA AGI Supremacy - 让 AGI 触手可及_