# Circom 集成设计文档

## 概述
将 Circom 生成的 Groth16 验证器集成到 Substrate 运行时，实现 GPU 矩阵乘法的零知识证明验证。

## 技术栈

### 1. Circom 电路
- **电路类型**：GPU 矩阵乘法验证
- **证明系统**：Groth16
- **输入**：矩阵 A, B, C 的哈希，计算时间
- **输出**：证明 π

### 2. 验证合约
- **语言**：Solidity (Circom 生成)
- **功能**：验证 Groth16 证明
- **接口**：`verifyProof(proof, inputs) → bool`

### 3. Substrate 集成
- **方式**：预编译合约或外部调用
- **位置**：`pallet-zk-compute` 中的验证器实现
- **数据流**：证明 → 验证合约 → 验证结果

## 电路设计

### 矩阵乘法电路
```
// 输入信号
signal input a_hash;      // 矩阵 A 的哈希
signal input b_hash;      // 矩阵 B 的哈希  
signal input c_hash;      // 矩阵 C 的哈希
signal input m, n, k;     // 矩阵维度
signal input timestamp;   // 计算时间戳

// 约束：C = A × B
// 实际实现需要更复杂的电路来验证矩阵乘法
```

### 电路参数
- **约束数量**：~1,000,000 (可优化)
- **证明生成时间**：~10 秒 (GPU 加速)
- **验证时间**：~100 ms (链上)

## 验证合约接口

### Solidity 接口
```solidity
interface IZkVerifier {
    function verifyProof(
        uint[2] memory a,
        uint[2][2] memory b,
        uint[2] memory c,
        uint[2] memory input
    ) external view returns (bool);
}
```

### Substrate 包装器
```rust
pub struct CircomVerifier {
    contract_address: H160,
    client: Arc<dyn EthereumInterface>,
}

impl<T: Config> ZkVerifier<T> for CircomVerifier {
    fn verify_proof(proof: &[u8], dimensions: &(u32, u32, u32)) -> Result<bool, Vec<u8>> {
        // 调用以太坊验证合约
        // 返回验证结果
    }
}
```

## 集成方案

### 方案 1：预编译合约 (推荐)
- 将 Circom 验证器编译为 Substrate 预编译合约
- 优点：高性能，无需外部依赖
- 缺点：需要定制开发

### 方案 2：外部调用
- 通过 Frontier EVM 调用验证合约
- 优点：兼容现有以太坊工具链
- 缺点：性能开销，外部依赖

### 方案 3：原生实现
- 在 Rust 中实现 Groth16 验证
- 优点：最佳性能
- 缺点：开发复杂度高

## 数据格式

### 证明格式 (Groth16)
```rust
struct Groth16Proof {
    a: [u8; 64],      // G1 point
    b: [u8; 128],     // G2 point  
    c: [u8; 64],      // G1 point
    inputs: Vec<u8>,  // 公共输入
}
```

### 验证输入
```rust
struct VerificationInput {
    a_hash: [u8; 32],
    b_hash: [u8; 32],
    c_hash: [u8; 32],
    m: u32,
    n: u32, 
    k: u32,
    timestamp: u64,
}
```

## 部署流程

### 1. 电路开发
```
1. 编写 Circom 电路
2. 编译电路生成 R1CS
3. 生成验证密钥 (vk) 和证明密钥 (pk)
```

### 2. 合约生成
```
1. 生成 Solidity 验证合约
2. 部署到测试网
3. 获取合约地址
```

### 3. Substrate 集成
```
1. 实现验证器包装器
2. 配置运行时
3. 测试集成
```

## 性能优化

### 证明生成优化
- **GPU 加速**：使用 CUDA 加速证明生成
- **批量验证**：支持批量证明验证
- **缓存机制**：缓存验证结果

### 链上优化
- **Gas 优化**：最小化验证合约 gas 消耗
- **存储优化**：压缩证明数据
- **并行验证**：支持并行验证多个证明

## 安全考虑

### 电路安全
1. **约束完整性**：确保电路正确约束矩阵乘法
2. **输入验证**：验证输入数据的有效性
3. **防伪证明**：防止伪造证明

### 合约安全
1. **重入攻击防护**：使用 checks-effects-interactions 模式
2. **整数溢出防护**：使用 SafeMath
3. **权限控制**：限制关键函数调用

### 集成安全
1. **数据验证**：验证证明格式和大小
2. **错误处理**：优雅处理验证失败
3. **日志记录**：记录所有验证操作

## 测试计划

### 单元测试
1. 电路功能测试
2. 验证合约测试
3. 集成包装器测试

### 集成测试
1. 端到端证明生成和验证
2. 性能基准测试
3. 错误场景测试

### 安全测试
1. 模糊测试
2. 形式化验证
3. 第三方审计

## 依赖项

### 开发依赖
1. `circom` - 电路编译器
2. `snarkjs` - ZK-SNARK 工具
3. `hardhat` - 合约开发框架

### 运行时依赖
1. `pallet-evm` - EVM 支持 (如果使用方案 2)
2. `sp-core` - 基础类型
3. `sp-io` - 运行时 IO

## 时间估算

| 阶段 | 时间 | 交付物 |
|------|------|--------|
| 电路开发 | 2 天 | Circom 电路文件 |
| 合约生成 | 1 天 | Solidity 验证合约 |
| Substrate 集成 | 2 天 | 验证器实现 |
| 测试优化 | 1 天 | 测试套件和文档 |
| **总计** | **6 天** | 完整集成 |

## 下一步行动

1. 选择集成方案 (推荐方案 1)
2. 开发 Circom 电路原型
3. 生成验证合约
4. 实现 Substrate 集成
5. 进行全面测试