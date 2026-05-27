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
