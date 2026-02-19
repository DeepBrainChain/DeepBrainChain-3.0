# ZK 验证 Pallet 设计文档

## 概述
ZK 验证 Pallet 是 DBC 3.0 的核心模块，负责验证 GPU 矩阵乘法的零知识证明。

## 功能需求

### 1. 证明提交
- 矿工提交 ZK 证明（Groth16 格式）
- 包含矩阵维度信息 (M, N, K)
- 包含计算时间戳

### 2. 证明验证
- 调用 Circom 生成的验证合约
- 验证证明的有效性
- 返回验证结果

### 3. 奖励计算
- 基础奖励：基于 A100 80GB 基准
- 奖励系数：NVLink 效率加成 (1.2x-1.5x)
- 最终奖励 = 基础奖励 × 系数

### 4. 状态管理
- 待验证任务队列
- 已验证任务历史
- 矿工信誉评分

## 数据结构

### ZkTask
```rust
struct ZkTask {
    task_id: u64,
    miner: AccountId,
    proof: Vec<u8>,
    dimensions: (u32, u32, u32),  // M, N, K
    status: ZkVerificationStatus,
    base_reward: Balance,
    multiplier_q100: u32,  // 放大100倍，150 = 1.5x
    submitted_at: BlockNumber,
}
```

### ZkVerificationStatus
```rust
enum ZkVerificationStatus {
    Pending,
    Verified,
    Failed,
}
```

## 接口设计

### Extrinsics
1. `submit_proof(proof, dimensions)` - 提交证明
2. `verify_task(task_id)` - 手动触发验证
3. `claim_reward(task_id)` - 领取奖励

### Storage
1. `Tasks` - 任务映射 (task_id → ZkTask)
2. `PendingTasks` - 待验证任务队列
3. `VerifiedTasks` - 已验证任务历史
4. `MinerScores` - 矿工信誉评分

### Events
1. `ProofSubmitted` - 证明已提交
2. `ProofVerified` - 证明已验证
3. `RewardClaimed` - 奖励已领取

## 验证流程

```
1. 矿工提交证明
   ↓
2. 验证器验证证明
   ↓
3. 计算奖励系数
   ↓
4. 更新任务状态
   ↓
5. 矿工领取奖励
```

## 奖励模型

### 基础奖励
- A100 80GB 基准：100 DBC/任务
- 根据矩阵规模调整：scale_factor = (M×N×K) / 1e6

### 效率系数
- NVLink 连接：1.2x
- NVLink 优化：1.5x
- 普通 PCIe：1.0x

### 最终奖励
```
reward = base_reward × scale_factor × efficiency_multiplier
```

## 安全考虑

1. **防重放攻击**：每个证明包含唯一 nonce
2. **防女巫攻击**：矿工信誉系统
3. **防 DoS 攻击**：提交押金机制
4. **验证时间限制**：超时自动失败

## 测试计划

### 单元测试
1. 证明提交测试
2. 验证逻辑测试
3. 奖励计算测试
4. 状态转换测试

### 集成测试
1. 端到端验证流程
2. 多矿工并发测试
3. 错误处理测试

## 部署计划

1. 开发环境测试
2. 测试网部署
3. 主网部署

## 依赖项

1. `pallet-balances` - 奖励支付
2. `pallet-timestamp` - 时间戳
3. Circom 验证合约 - ZK 证明验证