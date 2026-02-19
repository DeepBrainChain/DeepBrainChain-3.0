# DBC 3.0 升级总结与实施计划

> 本文档综合了 AI 战略分析、架构设计、经济模型、ZK 验证、算力池调度等所有研究成果，
> 作为 DBC 3.0 升级的统一参考。
>
> 来源：文博 AI Bot 2026-02-17 分析产出
> 整理日期：2026-02-19

---

## 目录

1. [现状分析](#1-现状分析)
2. [Substrate 升级计划](#2-substrate-升级计划)
3. [出块时间优化](#3-出块时间优化)
4. [Task Mode 挖矿经济模型](#4-task-mode-挖矿经济模型)
5. [算力池调度系统](#5-算力池调度系统)
6. [ZK 零知识证明验证](#6-zk-零知识证明验证)
7. [ETH AI Agent 协议支持](#7-eth-ai-agent-协议支持)
8. [X402 协议与稳定币](#8-x402-协议与稳定币)
9. [节点 Agent 接入协议](#9-节点-agent-接入协议)
10. [实施路线图](#10-实施路线图)
11. [风险与缓解](#11-风险与缓解)

---

## 1. 现状分析

### 技术基线

| 项目 | 当前状态 |
|------|---------|
| Substrate 版本 | `polkadot-v0.9.43` (已过时) |
| Rust 工具链 | `1.81.0` |
| 出块时间 | 6 秒 (`MILLISECS_PER_BLOCK = 6000`) |
| Epoch | 4 小时 |
| Era | 24 小时 |
| EVM | 自定义 `DBC-EVM` fork (Frontier, 同版本) |
| 自定义 Pallet | staking, assets, nfts (fork 自上游) |
| 预编译合约 | Bridge, DBCPrice, DLCPrice, MachineInfo |
| 价格预言机 | 单源滚动平均 (`MAX_LEN=64`) |
| 迁移状态 | `type Migrations = ()` (未使用正式迁移) |

### 核心依赖

- 自定义 fork pallet: `pallet-staking`, `pallet-assets`, `pallet-nfts`
- 自定义 DBC pallets: `rent-machine`, `online-profile`, `dbc-price-ocw`, `terminating-rental`
- EVM: `DeepBrainChain/DBC-EVM` fork

---

## 2. Substrate 升级计划

**目标**: 升级到 Polkadot-SDK 最新稳定线 (`polkadot-stable2509-5` 或更新)

### 主要破坏性变更

| 变更类别 | 影响 |
|---------|------|
| 依赖拓扑 | 旧 crate 级 git 依赖 → 新 umbrella crate 模式 |
| `SignedExtension` 废弃 | → `TransactionExtension` |
| `Currency` trait 废弃 | → `fungible` trait 族 |
| Metadata v15/v16 | 自定义 RPC/类型生成需验证 |
| 存储迁移 | 需采用 `VersionedMigration` 模式 |

### 分阶段策略

| 阶段 | 工作内容 | 预估周期 |
|------|---------|---------|
| **A: 构建图 rebase** | 替换 Substrate 依赖、升级 Rust 工具链、恢复编译 | 2-3 周 |
| **B: FRAME 兼容** | 迁移 SignedExtension → TransactionExtension, Currency → fungible | 3-5 周 |
| **C: 自定义 fork rebase** | 按顺序 rebase: staking → assets → nfts → DBC pallets | 4-8 周 |
| **D: Frontier/EVM rebase** | rebase DBC-EVM, 重新验证 eth_* RPC/tracing/precompile | 2-4 周 |
| **E: 迁移加固** | `VersionedMigration` + `try-runtime` + 权重基准测试 | 2-3 周 |

### 交付产物

- `MIGRATION_MATRIX.md` — crate 级别状态追踪
- `PALLET_COMPAT_REPORT.md` — 每个 pallet 的编译/API 差异
- `STORAGE_MIGRATION_PLAN.md` — 存储版本转换和回滚标准

---

## 3. 出块时间优化

### 结论: 6s → 1s 直接切换不可行

`runtime/src/constants.rs` 明确指出: slot duration/epoch 在链启动后变更可能导致出块中断。

### 技术影响分析

| 方面 | 风险 |
|------|------|
| 最终性 | BABE 频率 6x, GRANDPA 投票压力剧增 |
| 网络传播 | p95 传播+导入+提议必须 << 1s, 当前含 EVM 开销不够 |
| 验证器负载 | 多个 pallet 在 `on_initialize`/`on_finalize` 中返回 `Weight::zero()` 做无界迭代, 6x 放大 |
| 权重模型 | 当前模型允许 6s 块内 ~2s 计算, 1s 需要彻底重新设计 |

### 推荐方案

1. **近期**: 6s → 3s/2s (如果链迁移支持)
2. **1s 必须时**: 新网络/子网架构, 需满足:
   - 所有 hook 无界迭代 → 队列式增量处理
   - 强制非零 hook 权重
   - 减小区块大小和操作预算
   - 调优 GRANDPA/BABE 参数
   - 生产流量重放验证: 孤块率、最终性延迟、p95 导入时间、CPU 饱和度

---

## 4. Task Mode 挖矿经济模型

### 核心规则

```
矿工模式:
  - Task Mode: 运行指定 LLM 推理工作负载
  - Long-term Rental: 传统 GPU 租赁 (现有 rent-machine)

每 Era 奖励分配:
  - 70% → Task Mode 矿工
  - 30% → Long-term Rental 矿工

收入分成:
  - 15% 销毁 (burn)
  - 85% 矿工
```

### 计费公式

```
usd_value = input_tokens × input_price_per_1k / 1000
           + output_tokens × output_price_per_1k / 1000

dbc_due = DbcPrice::get_dbc_amount_by_value(usd_value)
burn    = dbc_due × 15%
miner   = dbc_due × 85%
```

### Pallet 设计 (`pallet-task-mode`)

**核心数据结构**:

```rust
// 任务定义 — 定义 LLM 模型及其价格
TaskDefinition { model_id, version, admin, input_price_usd_per_1k,
                 output_price_usd_per_1k, max_tokens_per_request, policy_cid, is_active }

// 任务订单 — 记录每次推理的计费
TaskOrder { order_id, task_id, customer, miner, input_tokens, output_tokens,
            dbc_price_snapshot, total_dbc_charged, dbc_burned, miner_payout,
            created_at, status, attestation_hash }

// 订单状态流
Pending → InProgress → Completed → Settled

// Era 统计 — 聚合每 Era 的 Task Mode 数据
EraTaskStats { total_charged, total_burned, total_miner_payout, completed_orders }
```

**Extrinsics**:

| 调用 | 功能 |
|------|------|
| `create_task_definition` | 注册 LLM 模型及价格 |
| `update_task_definition` | 更新价格/状态 |
| `create_task_order` | 创建推理订单 (获取价格、计算费用、冻结 DBC) |
| `mark_order_completed` | 矿工提交完成证明 |
| `settle_task_order` | 结算: burn 15% + 支付矿工 85% |

**集成点**:
- `pallet-dbc-price-ocw` — 获取实时 DBC 价格
- `pallet-online-profile` — 矿工信息
- 现有 `rent-machine` 的 reserve/transfer 模式

### 预言机加固需求

当前单源滚动平均不足以支撑生产计费:
- → 多源中位数喂价
- → 过期检查 + 偏差限制
- → 签名来源验证
- → 治理紧急开关

---

## 5. 算力池调度系统

### 三层架构

```
调度层 (Scheduler)          验证层 (Verifier)           奖励层 (Rewarder)
├─ 任务队列管理              ├─ ZK 证明验证              ├─ 奖励计算
├─ 矿工匹配算法              ├─ 算力质量评估              ├─ 代币分发
└─ 负载均衡                  └─ 信誉评分更新              └─ 激励优化
```

### 调度评分公式

```
score = (reputation_score / 100) × 0.4
      + (success_rate / 100)     × 0.3
      + (1 / normalized_price)   × 0.2
      + (nvlink_efficiency / 150) × 0.1
```

### 奖励公式

```
base_reward = A100_80GB_benchmark (100 DBC) × complexity_factor((M×N×K) / 1e6)
efficiency  = NVLink ? (1.2x ~ 1.5x) : 1.0x
final_reward = base_reward × efficiency × quality_bonus(0.8 ~ 1.2)
```

### 状态机

```
矿工: Registered → Active → Inactive → Deregistered
任务: Pending → Assigned → Computing → ProofSubmitted → Verifying → Completed/Failed
```

### 关键 Extrinsics

| 调用 | 功能 |
|------|------|
| `register_pool` | 注册算力池 (GPU 型号、NVLink、价格) |
| `submit_task` | 提交计算任务 |
| `submit_proof` | 矿工提交 ZK 证明 |
| `claim_reward` | 领取奖励 |
| `dispute_verification` | 申诉验证结果 |

---

## 6. ZK 零知识证明验证

### 验证目标

在不重新执行 AI 任务的情况下, 通过数学证明确保节点确实运行了指定模型并得到了正确结果。

### 技术选型

| 项目 | 选择 |
|------|------|
| 证明系统 | Groth16 (Plonk 备选, 支持通用设置) |
| 电路工具 | Circom |
| 链上集成 | `pallet-zk-verify` + 预编译合约 |

### 验证流程

```
1. 链端下发任务哈希
2. 节点本地计算结果 R 及 ZK 证明 π
3. 节点提交 (π, R) 至 pallet-compute-pool
4. Runtime 链上校验: 成功 → 发放奖励, 失败 → Slashing
```

### Circom 电路设计

```
输入信号: a_hash(矩阵A), b_hash(矩阵B), c_hash(矩阵C=A×B), m/n/k(维度), timestamp
约束: 验证 C = A × B
参数: ~1,000,000 约束, 证明生成 ~10s(GPU), 验证 ~100ms(链上)
```

### 集成方案 (推荐: 预编译合约)

```rust
// Substrate 验证器包装
impl<T: Config> ZkVerifier<T> for CircomVerifier {
    fn verify_proof(proof: &[u8], dimensions: &(u32, u32, u32)) -> Result<bool, Vec<u8>>
}
```

### 安全措施

- 防重放: 每个证明含唯一 nonce
- 防女巫: 硬件 UUID 绑定 + 信誉系统
- 防 DoS: 提交押金
- 验证超时自动失败

---

## 7. ETH AI Agent 协议支持

### 设计目标

让 DBC 成为以太坊原生 AI Agent 的一等执行和结算层。

### 标准兼容

| 标准 | 用途 |
|------|------|
| ERC-4337 | 账户抽象, UserOperation + EntryPoint |
| EIP-7702 | EOA 委托代码执行 / 批量 UX |
| EIP-5792 | 钱包批量调用 API + 能力发现 |
| ERC-4361 (SIWE) | 链下 Agent 服务的会话/身份认证 |

### 架构

**链上新 Pallet:**

| Pallet | 功能 |
|--------|------|
| `pallet-agent-task` | 注册 Task Mode 任务、存储承诺、跟踪结算 |
| `pallet-agent-attestation` | 签名结果证明、挑战窗口、Slashing |
| `AgentTaskPrecompile` | EVM 预编译, Solidity 低摩擦集成 |

**链下服务:**

| 组件 | 功能 |
|------|------|
| Bundler/Relayer | 处理 ERC-4337 UserOps, Paymaster |
| Agent Gateway | SIWE 认证 + 能力签发 + 任务提交/结果检索 |
| 可观测平面 | 确定性事件索引, Agent 订单生命周期审计 |

### 为什么适合 DBC

- 已有 EVM + 机器/租赁域状态在链上
- 已有预编译暴露机器和定价数据给 Solidity
- 现有 rent/billing 逻辑可泛化为 token 计量的 Agent 工作负载

---

## 8. X402 协议与稳定币

### X402 协议

HTTP 原生支付流: `402 Payment Required` + payment headers + facilitator 中介结算。
目标: AI Agent 微支付 + 稳定币结算。

### DBC 架构

| 组件 | 类型 | 功能 |
|------|------|------|
| x402 Gateway | 链下 | 返回 402 + 支付头, 接受签名, 提交结算证明 |
| `pallet-x402-settlement` | 链上 | 存储支付意图/nonce/防重放, 验证签名, 商户/矿工结算 |
| 稳定币轨道 | 链上 | 规范桥接稳定币 (DBC EVM), 或 pallet-assets 原生稳定资产 |

### 稳定币运营 (新 Pallet)

| Pallet | 功能 |
|--------|------|
| `pallet-stablecoin-treasury` | 储备金会计、铸币/销毁治理、储备证明检查点 |
| `pallet-stablecoin-risk` | 熔断器、资产上限、脱锚处理 |

---

## 9. 节点 Agent 接入协议

### 核心流程

```
1. 环境自检 — nvidia-smi 提取硬件 UUID 与规格
2. 算力评估 — 轻量 Transformer 推理测试 → TFLOPS
3. 身份生成 — Substrate 兼容账户私钥 (如不存在)
4. 报送上链 — 调用 register_node 注册
5. 心跳维持 — 每 100 区块发送心跳
```

### JSON-RPC 接口

| 方法 | 功能 |
|------|------|
| `node_getHardwareInfo` | 返回 GPU 详细参数 |
| `node_signAndSubmit` | 封装签名逻辑, 与主网交互 |

### 安全性

- 硬件绑定: 算力值与 GPU UUID 强绑定, 防虚假虚报
- ZK-Proof 预留: 接口预留, 用于接收链端下发的验证任务

---

## 10. 实施路线图

### 总览

```
Wave 0 (2周)     Wave 1 (6-10周)      Wave 2 (4-6周)       Wave 3 (5-8周)      Wave 4 (持续)
准备              SDK 升级              经济/Task 原语        ETH Agent + x402    性能/出块优化
├─ 确定版本线      ├─ 依赖+工具链 rebase  ├─ pallet-task-mode   ├─ 4337/7702/5792   ├─ 消除无界 hook
├─ 冻结 feature   ├─ 编译/API 恢复       ├─ 70/30 + 15/85     ├─ x402 网关+结算    ├─ 权重基准测试
└─ 兼容性仪表盘    ├─ fork rebase         ├─ 预言机加固          ├─ 稳定币组件        ├─ 评估 2s → 1s
                  └─ Frontier rebase     └─ 计费证明           └─ SIWE 认证         └─ SLO 门控
```

### 新 Pallet 清单

| Pallet | Wave | 功能 |
|--------|------|------|
| `pallet-task-mode` | 2 | Task Mode 挖矿计费与结算 |
| `pallet-zk-verify` | 2 | ZK 证明链上验证 |
| `pallet-compute-pool` | 2 | 算力池注册/调度/奖励 |
| `pallet-agent-task` | 3 | AI Agent 任务注册与跟踪 |
| `pallet-agent-attestation` | 3 | Agent 结果证明与挑战 |
| `pallet-x402-settlement` | 3 | X402 支付结算 |
| `pallet-stablecoin-treasury` | 3 | 稳定币储备管理 |
| `pallet-stablecoin-risk` | 3 | 稳定币风控 |

---

## 11. 风险与缓解

| 风险 | 等级 | 缓解措施 |
|------|------|---------|
| 自定义 fork 分歧 | 高 | 按顺序 rebase + 不变性测试 + 黄金状态重放 |
| 存储迁移安全 | 高 | `VersionedMigration` + `try-runtime` 预检 |
| 预言机操纵 (token 计费) | 中 | 多源中位数 + 过期边界 + 治理紧急开关 |
| 1s 出块不稳定 | 中 | 分阶段降低 + 严格 SLO 门控 |
| x402 facilitator 信任 | 低 | 验证器抽象 + 多 facilitator + 可审计证明 |

---

## 附录: 文档索引

| 文档 | 位置 | 说明 |
|------|------|------|
| 战略技术分析 (完整版) | `docs/v3-design/DBC_3.0_STRATEGY_ANALYSIS.md` | AI 生成的详细分析 (16KB) |
| 架构草案 v1 | `docs/v3-design/DBC-3.0-ARCH-v1.md` | 核心组件: DBC-Chain-v3, D-GVM, ZK-Verifier |
| 经济模型 | `docs/v3-design/DBC-3.0-ECONOMY.md` | 质押、奖励公式、Slashing |
| ZK 验证规格 | `docs/v3-design/ZK-VERIFICATION-SPEC.md` | Plonk/Groth16 技术选型与流程 |
| 节点 Agent 规格 | `docs/v3-design/agent_spec.md` | 节点自检、注册、心跳 |
| Task Mode 设计 | `docs/v3-design/pallet-task-mode-design.md` | 完整 Rust 数据结构与 Extrinsics |
| 算力池调度 | `docs/v3-design/compute-pool-scheduler.md` | 调度算法、奖励模型、状态机 |
| ZK Pallet 设计 | `docs/v3-design/zk-pallet-design.md` | Groth16 验证、奖励计算 |
| Circom 集成 | `docs/v3-design/circom-integration.md` | 电路设计、预编译方案 |

---

*Generated: 2026-02-19 | Source: 文博 AI Bot analysis on DBC 3.0 codebase*
