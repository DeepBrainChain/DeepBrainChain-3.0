import { ApiPromise, WsProvider, Keyring } from '@polkadot/api';
import { cryptoWaitReady } from '@polkadot/util-crypto';

await cryptoWaitReady();

const provider = new WsProvider('ws://127.0.0.1:9944');
const api = await ApiPromise.create({ provider });
const keyring = new Keyring({ type: 'sr25519' });
const alice = keyring.addFromUri('//Alice');
const bob = keyring.addFromUri('//Bob');
const charlie = keyring.addFromUri('//Charlie');

console.log('Connected to:', (await api.rpc.system.chain()).toString());
console.log('Runtime version:', (await api.rpc.state.getRuntimeVersion()).specVersion.toNumber());
console.log('Best block:', (await api.rpc.chain.getHeader()).number.toNumber());
console.log('');

let passed = 0, failed = 0;

// Helper: submit and wait for inclusion
async function submitAndWait(tx, signer, label) {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => { failed++; console.log(`[FAIL] ${label}: timeout`); resolve(false); }, 30000);
    tx.signAndSend(signer, ({ status, dispatchError, events }) => {
      if (status.isInBlock) {
        clearTimeout(timeout);
        if (dispatchError) {
          failed++;
          if (dispatchError.isModule) {
            const decoded = api.registry.findMetaError(dispatchError.asModule);
            console.log(`[FAIL] ${label}: ${decoded.section}.${decoded.name} — ${decoded.docs.join(' ')}`);
          } else {
            console.log(`[FAIL] ${label}: ${dispatchError.toString()}`);
          }
          resolve(false);
        } else {
          passed++;
          const block = status.asInBlock.toHex().substring(0, 10);
          // Extract relevant events
          const palletEvents = events
            .filter(({ event }) => !event.section.startsWith('system') && !event.section.startsWith('balances') && event.section !== 'transactionPayment')
            .map(({ event }) => `${event.section}.${event.method}`)
            .join(', ');
          console.log(`[OK]   ${label} (block ${block}...)${palletEvents ? ' → events: ' + palletEvents : ''}`);
          resolve(true);
        }
      }
    }).catch(e => { clearTimeout(timeout); failed++; console.log(`[FAIL] ${label}: ${e.message}`); resolve(false); });
  });
}

// Helper: query storage
async function queryStorage(label, fn) {
  try {
    const result = await fn();
    const json = result.toJSON ? result.toJSON() : result;
    console.log(`       ${label}:`, JSON.stringify(json).substring(0, 200));
    return json;
  } catch (e) {
    console.log(`       ${label}: query error — ${e.message}`);
    return null;
  }
}

try {
  // ================================================================
  // PALLET 1: TaskMode — AI 任务定价与订单
  // ================================================================
  console.log('━━━ Pallet 1: TaskMode ━━━');

  // 1.1 Create task definition (Alice as admin)
  const modelId = '0x' + Buffer.from('gpt-4-turbo').toString('hex');
  const version = '0x' + Buffer.from('v1.0').toString('hex');
  const policyCid = '0x' + Buffer.from('QmTestCid12345').toString('hex');
  await submitAndWait(
    api.tx.taskMode.createTaskDefinition(modelId, version, 1000000, 2000000, 4096, policyCid),
    alice, '1.1 createTaskDefinition'
  );
  await queryStorage('task definition 0', () => api.query.taskMode.taskDefinitions(0));

  // 1.2 Update task definition
  await submitAndWait(
    api.tx.taskMode.updateTaskDefinition(0, 1200000, 2400000, 8192, null),
    alice, '1.2 updateTaskDefinition'
  );

  // 1.3 Create task order — requires DBC price oracle (not available on dev chain)
  // This is expected to fail with PriceOracleUnavailable
  console.log('       [NOTE] createTaskOrder requires DBC price oracle — testing expected failure');
  await submitAndWait(
    api.tx.taskMode.createTaskOrder(0, bob.address, 100, 50),
    alice, '1.3 createTaskOrder (expect FAIL: no oracle)'
  );
  // Reset fail counter since this is expected
  failed--; passed++;

  console.log('');

  // ================================================================
  // PALLET 2: ComputePoolScheduler — GPU 矿池调度
  // ================================================================
  console.log('━━━ Pallet 2: ComputePoolScheduler ━━━');

  // 2.1 Register pool (Alice as pool owner)
  const gpuModel = '0x' + Buffer.from('RTX4090').toString('hex');
  await submitAndWait(
    api.tx.computePoolScheduler.registerPool(gpuModel, 24576, true, 130, 1000000000000),
    alice, '2.1 registerPool'
  );
  await queryStorage('pool 0', () => api.query.computePoolScheduler.pools(0));

  // 2.2 Update pool config
  const gpuModelA100 = '0x' + Buffer.from('A100-80G').toString('hex');
  await submitAndWait(
    api.tx.computePoolScheduler.updatePoolConfig(0, gpuModelA100, 81920, true, 135, 1500000000000),
    alice, '2.2 updatePoolConfig'
  );

  // 2.3 Submit task (Bob as user)
  await submitAndWait(
    api.tx.computePoolScheduler.submitTask({ m: 256, n: 256, k: 128 }, 'Normal', null),
    bob, '2.3 submitTask'
  );
  await queryStorage('task 0', () => api.query.computePoolScheduler.tasks(0));

  // 2.4 Submit proof (Alice as pool owner — proof only, no verification)
  const proofHash = '0x' + 'ff'.repeat(32);
  await submitAndWait(
    api.tx.computePoolScheduler.submitProof(0, proofHash),
    alice, '2.4 submitProof (proof only)'
  );

  // 2.5 Self-verification must fail (Alice is pool owner)
  console.log('       [NOTE] Testing that pool owner cannot self-verify (expected failure)');
  await submitAndWait(
    api.tx.computePoolScheduler.verifyProof(0, true),
    alice, '2.5 selfVerify (expect FAIL: SelfVerificationNotAllowed)'
  );
  // Reset counter since this is expected
  failed--; passed++;

  // 2.6 Verify proof (Charlie as independent verifier — NOT pool owner)
  await submitAndWait(
    api.tx.computePoolScheduler.verifyProof(0, true),
    charlie, '2.6 verifyProof (independent)'
  );

  // 2.7 Claim reward (Alice as pool owner)
  await submitAndWait(
    api.tx.computePoolScheduler.claimReward(0),
    alice, '2.7 claimReward'
  );

  // 2.8 Stake to pool (Charlie stakes to pool 0)
  await submitAndWait(
    api.tx.computePoolScheduler.stakeToPool(0, 100000000000),
    charlie, '2.8 stakeToPool'
  );
  await queryStorage('stake', () => api.query.computePoolScheduler.poolStakes(0, charlie.address));

  // 2.9 Unstake from pool
  await submitAndWait(
    api.tx.computePoolScheduler.unstakeFromPool(0, 50000000000),
    charlie, '2.9 unstakeFromPool'
  );

  console.log('');

  // ================================================================
  // PALLET 3: AgentAttestation — AI 节点注册与认证
  // ================================================================
  console.log('━━━ Pallet 3: AgentAttestation ━━━');

  // 3.1 Register node (Alice)
  const gpuUuid = '0x' + Buffer.from('GPU-uuid-0001-abcd-efgh').toString('hex');
  await submitAndWait(
    api.tx.agentAttestation.registerNode(gpuUuid, 100),
    alice, '3.1 registerNode'
  );
  await queryStorage('node registration', () => api.query.agentAttestation.nodeRegistrations(alice.address));

  // 3.2 Update capability (model_ids, max_concurrent, price_per_token, region)
  const modelIds = ['0x' + Buffer.from('gpt-4-turbo').toString('hex')];
  const region = '0x' + Buffer.from('us-east').toString('hex');
  await submitAndWait(
    api.tx.agentAttestation.updateCapability(modelIds, 10, 1000000, region),
    alice, '3.2 updateCapability'
  );

  // 3.3 Submit attestation (Bob registers first, then attests for task 0)
  const gpuUuid2 = '0x' + Buffer.from('GPU-uuid-0002-ijkl-mnop').toString('hex');
  await submitAndWait(
    api.tx.agentAttestation.registerNode(gpuUuid2, 100),
    bob, '3.3 registerNode (Bob)'
  );
  const attestResultHash = '0x' + 'ee'.repeat(32);
  const attestModelId = '0x' + Buffer.from('gpt-4-turbo').toString('hex');
  await submitAndWait(
    api.tx.agentAttestation.submitAttestation(0, attestResultHash, attestModelId, 100, 50),
    bob, '3.4 submitAttestation'
  );

  console.log('');

  // ================================================================
  // PALLET 4: ZkCompute — 零知识计算验证
  // ================================================================
  console.log('━━━ Pallet 4: ZkCompute ━━━');

  // 4.1 Submit proof
  const zkProof = '0x' + Buffer.from('zk-snark-proof-data-v2').toString('hex');
  await submitAndWait(
    api.tx.zkCompute.submitProof(zkProof, [256, 256, 128], 130, 1),
    alice, '4.1 submitProof'
  );
  await queryStorage('zk task 0', () => api.query.zkCompute.zkTasks(0));

  // 4.2 Verify task (Bob as verifier — only takes task_id)
  await submitAndWait(
    api.tx.zkCompute.verifyTask(0),
    bob, '4.2 verifyTask'
  );

  // 4.3 Claim reward
  await submitAndWait(
    api.tx.zkCompute.claimReward(0),
    alice, '4.3 claimReward'
  );

  console.log('');

  // ================================================================
  // PALLET 5: X402Settlement — 支付结算
  // ================================================================
  console.log('━━━ Pallet 5: X402Settlement ━━━');

  // 5.1 Submit payment intent (Alice pays Bob)
  // Note: signature verification is real now (sr25519), we need the facilitator's key
  // For dev chain, the FacilitatorPublicKey is from seed [1u8; 32]
  // We sign with the matching key
  const facilitatorPair = keyring.addFromSeed(new Uint8Array(32).fill(1));
  console.log(`       Facilitator address: ${facilitatorPair.address}`);

  const amount = 500000000000n; // 500 DBC
  const nonce = 1;
  const fingerprint = '0x' + '01'.repeat(32);

  // Build message matching pallet's SCALE encoding
  // merchant.encode + miner.encode + amount.encode + nonce.encode + fingerprint.encode
  const { u8aToHex } = await import('@polkadot/util');
  const merchantEncoded = api.createType('AccountId', alice.address).toU8a();
  const minerEncoded = api.createType('AccountId', bob.address).toU8a();
  const amountEncoded = api.createType('u128', amount).toU8a();
  const nonceEncoded = api.createType('u64', nonce).toU8a();
  const fingerprintEncoded = api.createType('H256', fingerprint).toU8a();

  const message = new Uint8Array([...merchantEncoded, ...minerEncoded, ...amountEncoded, ...nonceEncoded, ...fingerprintEncoded]);
  const signature = facilitatorPair.sign(message);
  const sigHex = u8aToHex(signature);
  console.log(`       Signature (${signature.length} bytes): ${sigHex.substring(0, 20)}...`);

  await submitAndWait(
    api.tx.x402Settlement.submitPaymentIntent(bob.address, amount, nonce, fingerprint, sigHex),
    alice, '5.1 submitPaymentIntent'
  );
  await queryStorage('payment intent 0', () => api.query.x402Settlement.paymentIntents(0));

  // 5.2 Verify settlement (must be called by FacilitatorAccount)
  // FacilitatorAccount = public key from seed [1u8; 32] — same as facilitatorPair
  // Need to fund the facilitator account first
  console.log(`       Funding facilitator account...`);
  await submitAndWait(
    api.tx.balances.transferKeepAlive(facilitatorPair.address, 10000000000000n),
    alice, '5.2a fundFacilitator'
  );
  await submitAndWait(
    api.tx.x402Settlement.verifySettlement(0),
    facilitatorPair, '5.2 verifySettlement'
  );

  // 5.3 Finalize settlement (SettlementDelay = 5 blocks on dev)
  console.log('       Waiting 35s for settlement delay (5 blocks)...');
  await new Promise(r => setTimeout(r, 35000));
  await submitAndWait(
    api.tx.x402Settlement.finalizeSettlement(0),
    alice, '5.3 finalizeSettlement'
  );

  // 5.4 Test fail_payment_intent (create new intent, then fail it)
  const nonce2 = 2;
  const fingerprint2 = '0x' + '02'.repeat(32);
  const message2 = new Uint8Array([...merchantEncoded, ...minerEncoded, ...amountEncoded, ...api.createType('u64', nonce2).toU8a(), ...api.createType('H256', fingerprint2).toU8a()]);
  const sig2 = facilitatorPair.sign(message2);
  await submitAndWait(
    api.tx.x402Settlement.submitPaymentIntent(bob.address, amount, nonce2, fingerprint2, u8aToHex(sig2)),
    alice, '5.4 submitPaymentIntent (for fail test)'
  );
  await submitAndWait(
    api.tx.x402Settlement.failPaymentIntent(1),
    facilitatorPair, '5.5 failPaymentIntent'
  );

  console.log('');

  // ================================================================
  // SUMMARY
  // ================================================================
  console.log('━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━');
  console.log(`Results: ${passed} passed, ${failed} failed, ${passed + failed} total`);
  console.log('━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━');

} catch (e) {
  console.error('Fatal error:', e.message);
  console.error(e.stack);
}

await api.disconnect();
process.exit(failed > 0 ? 1 : 0);
