# DBC 3.0 节点侧接入协议 (Agent Spec v1.0)

## 1. 核心流程
1. **环境自检**: Agent 启动后调用 `nvidia-smi` 提取硬件 UUID 与规格。
2. **算力评估**: 运行轻量级 Transformer 模型推理测试，获取 TFLOPS 数据。
3. **身份生成**: 节点自主生成 Substrate 兼容的账户私钥（如果不存在）。
4. **报送上链**: 调用 `register_node` 接口进行注册。
5. **心跳维持**: 每 100 个区块发送一次心跳，证明在线状态。

## 2. 接口定义 (JSON-RPC)
- `node_getHardwareInfo`: 返回 GPU 详细参数。
- `node_signAndSubmit`: 封装签名逻辑，与 DBC 3.0 主网交互。

## 3. 安全性
- **硬件绑定**: 算力值与 GPU UUID 强绑定，防止虚假虚报。
- **ZK-Proof 预留**: 预留接口，用于接收链端下发的验证任务。
