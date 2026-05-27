# APEX-SearchSkill 融合架构设计文档
**版本**: v1.0 | **日期**: 2026-05-20 | **状态**: 核心实现完成

---

## 一、核心设计目标

将 SearchSkill 的**技能库驱动检索**范式与 Hermes Claw 底层机制融合，构建**可控化、高精度、自演进**的智能检索增强系统。

### 解决的问题
- 大模型隐式随机搜索 → 规范化 Select-Read-Act 三段执行
- 检索质量无评估 → 随机森林多树投票验证
- 技能迭代依赖人工 → Gini/IG 自动选择最优变异路径
- Python 粘合层过重 → Rust/C/Go 核心，Python 薄胶水

---

## 二、核心技术架构

### 2.1 模块层次

```
┌─────────────────────────────────────────────────────────────┐
│  Hermes Agent (Python)                                        │
│  run_agent.py / conversation_loop.py / tool_dispatch         │
├─────────────────────────────────────────────────────────────┤
│  Python Glue Layer (粘合层)                                  │
│  search_skill_core/                                          │
│    skill_pipeline.py      <- SRA 执行管道                    │
│    hermes_integration.py  <- Hermes Skill System 桥接        │
├─────────────────────────────────────────────────────────────┤
│  Rust Core (可选, 核心逻辑)                                  │
│  search_skill_core/src/                                      │
│    gene.rs     <- 基因节点 (含 Gini/IG 元数据)               │
│    skill_bank.rs <- 演进式 SkillBank 知识库                 │
│    selector.rs <- Gini/IG 选择器 (ΔGini, IG 公式)           │
│    judge.rs    <- 随机森林判题器 (多数投票/软投票/OOB)       │
│    sra.rs      <- Select-Read-Act 三段管道                   │
│    ffi.rs      <- PyO3 绑定                                  │
├─────────────────────────────────────────────────────────────┤
│  External Services                                           │
│  GPT API (freemodel.dev) | Web Search | Hermes Skills DB     │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 文件结构

```
C:\Users\Administrator\AppData\Local\hermes\search_skill_core\
├── src/                        # Rust 核心 (可选)
│   ├── lib.rs                  # 模块入口
│   ├── gene.rs                 # Gene 节点定义
│   ├── skill_bank.rs           # SkillBank 知识库
│   ├── selector.rs             # Gini/IG 选择器
│   ├── judge.rs                # 随机森林判题器
│   ├── sra.rs                  # SRA 执行管道
│   ├── ffi.rs                  # PyO3 FFI 绑定
│   └── Cargo.toml
├── __init__.py                 # Python 包入口
├── skill_pipeline.py           # SRA Python 实现 (主逻辑)
├── hermes_integration.py       # Hermes Skill System 桥接
└── README.md                   # 快速开始文档

~/.hermes/skills/search-skill/  # 同步输出的 Hermes Skill 目录
```

---

## 三、核心公式

### 3.1 Gini 选择器

```rust
// Gini 不纯度: 衡量数据集纯度 (0=完全纯, 0.5=最大不纯)
Gini(D) = 1 - Σ p²

// Gini 增益: 评估分裂质量, 选最大增益路径
ΔGini = Gini_parent - (N_L/N)·Gini_L - (N_R/N)·Gini_R

// 信息增益: 熵的角度评估分裂
IG = H_parent - Σ (N_v/N)·H_v
    where H(D) = -Σ p·log₂(p)
```

### 3.2 随机森林判题器

```rust
// Bootstrap 采样: 63.2% 概率被选中, 36.8% OOB
// 每棵树独立采样, 袋外数据用于验证

// 硬投票: 多数决定最终类别
ŷ = argmax_c Σ I(h_b(x) = c)

// 软投票: 概率加权
p̂_c = (1/B) Σ p_{b,c}

// OOB 准确率: 袋外数据验证
OOB_acc = (正确预测数) / (总样本数)
```

---

## 四、Select-Read-Act 三段执行范式

### 4.1 Select (技能选择)

```
输入: User Query + Context
处理: 
  1. 难度分级 (Simple/Moderate/Complex)
  2. 从 SkillBank 召回候选基因
  3. Gini 排序 + 触发条件匹配 → Top-3 技能
输出: [(Gene, score), ...]
```

**难度分级规则**:
- Complex: 含 "why/therefore/because/为什么/推理"
- Moderate: 含 "verify/compare/confirm/比较/确认"
- Simple: 其他

### 4.2 Read (规则读取)

```
输入: 选中的 Gene + Query + Context
处理: 读取技能的 trigger_conditions + output_schema
      生成标准化的检索指令字符串
输出: [SKILL: xxx] [TRIGGER: xxx] [QUERY: xxx] [CONTEXT: xxx] [OUTPUT: schema]
```

### 4.3 Act (执行)

```
输入: SRA 指令
处理: Python 调用外部搜索 API (Web Search / GPT)
输出: 原始检索结果
```

### 4.4 Judge (验证)

```
输入: Act 输出 + Gene
处理: RFJudge 多树投票验证质量
      更新 Gene 的 oob_accuracy + confidence
输出: 验证通过的检索结果 + 更新的 Gene 适应度
```

---

## 五、SkillBank 演进机制

### 5.1 基因生命周期

```
新增 →  Bootstrapped →  RF评估 →  写入Bank →  定期蒸馏 →  淘汰/保留
          ↓                              ↓
       63.2%采样                      Gini排序
       36.8%OOB                       Top-K保留
```

### 5.2 知识蒸馏

```python
# 当 Bank 过大或质量下降时触发
def distill(threshold_confidence=0.3, threshold_oob=0.3):
    genes.retain(g => g.confidence >= tc OR g.oob_accuracy >= to)
    # 重建索引, 版本号+1
```

### 5.3 自演进条件

- 每轮 ApexSpiral 学习结束后触发 SkillBank 评估
- 高适应度基因同步到 ~/.hermes/skills/search-skill/
- 低适应度基因标记为 retired, 不参与 Select

---

## 六、Hermes Claw 融合点

### 6.1 注入位置

| Hermes 模块 | 融合方式 | 作用 |
|---|---|---|
| agent/tool_dispatch_helpers.py | 包装 web_search 调用 | SRA 执行前先 Select-Read |
| agent/skill_utils.py | SkillBank 双向同步 | 已有 Hermes Skill ↔ SearchSkill 互通 |
| model_tools.py | discover_builtin_tools() | 注册 search_skill_core 工具 |
| cron/jobs.py | ApexSpiral 进化触发 | 定期蒸馏 + 同步 |

### 6.2 Hermes Skill 导入

```python
HermesSkillBridge._load_hermes_skills()
# 扫描 ~/.hermes/skills/ 下所有 SKILL.md
# 解析 frontmatter → GeneRecord → SkillBank.add_gene()
```

### 6.3 SearchSkill 导出

```python
HermesSkillBridge.sync_to_hermes_skills()
# 高适应度技能 → ~/.hermes/skills/search-skill/<name>/SKILL.md
# Hermes Agent 重启后自动加载为内置 Skill
```

---

## 七、freemodel.dev GPT 集成

### 7.1 注册状态

- freemodel.dev 可访问 (HTTP 200)
- 注册流程: `/api/auth/send-otp` (邮箱) → `/api/auth/verify-otp`
- **阻塞点**: 需要真实手机/邮箱接收 OTP, 当前环境无法接收

### 7.2 API 端点

```
POST /api/auth/send-otp    {"email": "xxx"}
POST /api/auth/verify-otp  {"email": "xxx", "code": "123456"}
GET  /api/keys             (需认证) -> 创建 API Key
POST /api/keys             {"name": "xxx"} (需认证)
```

### 7.3 集成方案

```python
# freemodel GPT 调用 (注册后)
import openai
client = openai.OpenAI(
    base_url="https://freemodel.dev/v1",
    api_key="fk-xxx"  # 注册后获取
)
# 标准 OpenAI ChatCompletion 格式调用
```

---

## 八、已知问题与解决

### 8.1 evolver.py Cron 失败

**症状**: 所有 subprocess 命令返回 0 字节, DNS 解析到 198.18.x.x (AnySearch)

**根因**: Windows Sandbox 环境的网络层 DNS 劫持, 阻止所有外网 HTTP 请求

**解决**: 
1. evolver.py 脚本本身正常, 真实环境可运行
2. cron job 输出 0 字节 → 改用 `deliver: "local"` 避免 origin 投递
3. 网络层问题需联系 AnySearch 技术支持

### 8.2 AnySearch 网络不通

**症状**: DNS 解析到 198.18.x.x (私有地址), TCP 握手挂死

**根因**: AnySearch 服务可能在私有网络或需要专属客户端建立隧道

**解决**: 等待 AnySearch 技术支持确认

---

## 九、后续计划

### Phase 1: 融合验证 (今天)
- [x] SearchSkill Pipeline Python 实现
- [x] Rust 核心骨架 (可选)
- [x] HermesSkillBridge 桥接
- [ ] 实际 Query 测试 (freemodel 注册后)
- [ ] Hermes Agent 注入点改造

### Phase 2: 自进化闭环 (本周)
- [ ] evolver.py 修复 (网络问题绕过)
- [ ] ApexSpiral GeneBank → SearchSkill SkillBank 同步
- [ ] Gini 选择器在基因变异路径上的实际应用
- [ ] cron job 稳定输出

### Phase 3: Rust 核心编译 (可选)
- [ ] cargo build --features python
- [ ] PyO3 绑定验证
- [ ] 性能对比基准测试

---

## 十、核心文件清单

| 文件 | 大小 | 说明 |
|---|---|---|
| search_skill_core/skill_pipeline.py | ~15KB | SRA Python 主逻辑 |
| search_skill_core/hermes_integration.py | ~5KB | Hermes 桥接 |
| search_skill_core/src/gene.rs | ~5KB | Gene 节点定义 |
| search_skill_core/src/selector.rs | ~4KB | Gini/IG 选择器 |
| search_skill_core/src/judge.rs | ~5KB | RF 随机森林判题器 |
| search_skill_core/src/skill_bank.rs | ~4KB | 演进式技能库 |
| search_skill_core/src/sra.rs | ~4KB | SRA 管道 |
| search_skill_core/src/ffi.rs | ~4KB | PyO3 FFI |
| search_skill_core/src/Cargo.toml | ~200B | Rust 构建配置 |
