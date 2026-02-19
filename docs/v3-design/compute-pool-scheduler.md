# 算力池调度设计文档

## 概述
算力池调度系统是 DBC 3.0 的核心组件，负责自动化分配计算任务、验证算力证明、分发奖励。

## 架构设计

### 三层架构
```
1. 调度层 (Scheduler)
   - 任务队列管理
   - 矿工匹配算法
   - 负载均衡

2. 验证层 (Verifier)  
   - ZK 证明验证
   - 算力质量评估
   - 信誉评分更新

3. 奖励层 (Rewarder)
   - 奖励计算
   - 代币分发
   - 激励优化
```

## 核心功能

### 1. 算力池注册
- 矿工注册算力池
- 提交硬件配置 (GPU 型号、内存、NVLink)
- 设置服务参数 (价格、可用时间)

### 2. 任务调度
- 用户提交计算任务
- 调度器匹配最优矿工
- 分配任务并跟踪进度

### 3. 证明验证
- 矿工提交 ZK 证明
- 验证器验证证明有效性
- 更新矿工信誉评分

### 4. 奖励分发
- 基于验证结果计算奖励
- 应用 NVLink 效率系数
- 自动分发代币奖励

## 数据结构

### ComputePool
```rust
struct ComputePool {
    pool_id: u64,
    owner: AccountId,
    gpu_model: String,
    gpu_memory_gb: u32,
    has_nvlink: bool,
    nvlink_efficiency: u32,  // 120-150 (1.2x-1.5x)
    price_per_task: Balance,
    available_from: BlockNumber,
    available_to: BlockNumber,
    status: PoolStatus,
    total_tasks: u32,
    success_rate: u32,  // 百分比
    reputation_score: u32,
}
```

### ComputeTask
```rust
struct ComputeTask {
    task_id: u64,
    user: AccountId,
    pool_id: u64,
    dimensions: (u32, u32, u32),  // M, N, K
    priority: TaskPriority,
    status: TaskStatus,
    submitted_at: BlockNumber,
    started_at: Option<BlockNumber>,
    completed_at: Option<BlockNumber>,
    proof_hash: Option<[u8; 32]>,
    verification_result: Option<bool>,
    reward_amount: Option<Balance>,
}
```

## 调度算法

### 匹配策略
1. **硬件匹配**：GPU 型号和内存满足要求
2. **价格优先**：选择性价比最高的矿工
3. **信誉优先**：高信誉矿工优先分配
4. **负载均衡**：避免单个矿工过载

### 评分公式
```
score = 
  (reputation_score / 100) × 0.4 +
  (success_rate / 100) × 0.3 +
  (1 / normalized_price) × 0.2 +
  (nvlink_efficiency / 150) × 0.1
```

## 奖励模型

### 基础奖励公式
```
base_reward = benchmark_reward × complexity_factor

其中：
- benchmark_reward: A100 80GB 基准奖励 (100 DBC)
- complexity_factor: (M×N×K) / 1e6
```

### 效率系数
```
efficiency_multiplier = 
  if has_nvlink {
    nvlink_efficiency / 100  // 1.2-1.5
  } else {
    1.0
  }
```

### 最终奖励
```
final_reward = base_reward × efficiency_multiplier × quality_bonus

其中：
- quality_bonus: 基于验证质量的奖励系数 (0.8-1.2)
```

## 状态机

### 矿工状态
```
Registered → Active → Inactive → Deregistered
```

### 任务状态
```
Pending → Assigned → Computing → ProofSubmitted → Verifying → Completed/Failed
```

## 安全机制

### 防作弊措施
1. **任务超时**：超时任务自动失败
2. **证明验证**：所有计算必须提供 ZK 证明
3. **信誉系统**：低信誉矿工减少任务分配
4. **押金机制**：矿工需要质押保证金

### 经济安全
1. **奖励锁定**：验证通过后解锁奖励
2. **惩罚机制**：恶意行为扣除保证金
3. **争议解决**：用户可申诉错误验证

## 性能优化

### 调度优化
- **批量处理**：批量分配任务减少链上调用
- **缓存机制**：缓存矿工信息和匹配结果
- **异步验证**：验证过程异步执行不阻塞调度

### 存储优化
- **状态压缩**：使用紧凑数据结构
- **历史归档**：定期归档历史数据
- **索引优化**：为常用查询创建索引

## 接口设计

### Extrinsics
1. `register_pool(config, deposit)` - 注册算力池
2. `update_pool_config(pool_id, config)` - 更新配置
3. `deregister_pool(pool_id)` - 注销算力池
4. `submit_task(dimensions, priority)` - 提交计算任务
5. `submit_proof(task_id, proof)` - 提交计算证明
6. `claim_reward(task_id)` - 领取任务奖励
7. `dispute_verification(task_id)` - 申诉验证结果

### Storage
1. `Pools` - 算力池注册表
2. `Tasks` - 计算任务表
3. `PoolTasks` - 矿工任务分配
4. `MinerReputation` - 矿工信誉评分
5. `Rewards` - 待领取奖励

### Events
1. `PoolRegistered` - 算力池已注册
2. `TaskSubmitted` - 任务已提交
3. `TaskAssigned` - 任务已分配
4. `ProofSubmitted` - 证明已提交
5. `ProofVerified` - 证明已验证
6. `RewardAvailable` - 奖励可领取

## 测试计划

### 单元测试
1. 调度算法测试
2. 奖励计算测试
3. 状态转换测试
4. 错误处理测试

### 集成测试
1. 端到端任务流程测试
2. 多矿工并发测试
3. 负载压力测试
4. 故障恢复测试

### 经济模型测试
1. 激励兼容性测试
2. 博弈论分析
3. 经济攻击模拟

## 部署策略

### 阶段部署
1. **Alpha 阶段**：单矿工测试，基础功能验证
2. **Beta 阶段**：多矿工测试，调度算法优化
3. **生产阶段**：全功能部署，经济模型激活

### 升级策略
1. **无状态迁移**：通过 runtime 升级添加新功能
2. **数据迁移**：使用迁移 pallet 处理数据结构变更
3. **渐进式部署**：逐步增加功能和矿工规模

## 监控和运维

### 关键指标
1. **调度成功率**：任务成功分配比例
2. **验证通过率**：证明验证成功比例
3. **平均响应时间**：任务提交到完成时间
4. **矿工利用率**：矿工算力使用率
5. **奖励分发效率**：奖励计算和分发速度

### 告警机制
1. **调度异常**：长时间无任务分配
2. **验证失败率过高**：连续验证失败
3. **奖励分发延迟**：奖励未及时分发
4. **系统负载过高**：接近容量上限

## 依赖关系

### 内部依赖
1. `pallet-zk-compute` - ZK 证明验证
2. `pallet-balances` - 代币管理和奖励分发
3. `pallet-timestamp` - 时间戳和超时管理

### 外部依赖
1. **链下调度器**：优化调度决策 (可选)
2. **监控服务**：系统监控和告警
3. **数据分析**：性能分析和优化

## 时间估算

| 阶段 | 时间 | 交付物 |
|------|------|--------|
| 核心数据结构 | 1 天 | 数据结构定义和存储 |
| 调度算法实现 | 2 天 | 匹配算法和分配逻辑 |
| 奖励系统实现 | 1 天 | 奖励计算和分发 |
| 安全机制实现 | 1 天 | 防作弊和经济安全 |
| 测试和优化 | 2 天 | 测试套件和性能优化 |
| **总计** | **7 天** | 完整调度系统 |

## 下一步行动

1. 实现核心数据结构和存储
2. 开发调度算法原型
3. 集成 ZK 验证模块
4. 实现奖励分发逻辑
5. 进行全面测试和优化