import { ApiPromise, WsProvider, Keyring } from '@polkadot/api';

const provider = new WsProvider('ws://127.0.0.1:9944');
const api = await ApiPromise.create({ provider });
const keyring = new Keyring({ type: 'sr25519' });
const alice = keyring.addFromUri('//Alice');
const bob = keyring.addFromUri('//Bob');

console.log('Connected to chain:', (await api.rpc.system.chain()).toString());
console.log('');

// Helper: submit and wait for inclusion
async function submitAndWait(tx, signer, label) {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(label + ': timeout')), 30000);
    tx.signAndSend(signer, ({ status, dispatchError, events }) => {
      if (status.isInBlock) {
        clearTimeout(timeout);
        if (dispatchError) {
          if (dispatchError.isModule) {
            const decoded = api.registry.findMetaError(dispatchError.asModule);
            console.log('[FAIL]', label, ':', decoded.section + '.' + decoded.name, '-', decoded.docs.join(' '));
          } else {
            console.log('[FAIL]', label, ':', dispatchError.toString());
          }
        } else {
          console.log('[OK]', label, '- included in block', status.asInBlock.toHex().substring(0, 10) + '...');
        }
        resolve();
      }
    }).catch(e => { clearTimeout(timeout); reject(e); });
  });
}

try {
  // === 1. TaskMode: create_task_definition ===
  console.log('--- Test 1: TaskMode ---');
  const modelId = '0x' + Buffer.from('gpt-4-turbo').toString('hex');
  const version = '0x' + Buffer.from('v1.0').toString('hex');
  const policyCid = '0x' + Buffer.from('QmTest').toString('hex');
  await submitAndWait(
    api.tx.taskMode.createTaskDefinition(modelId, version, 1000000, 2000000, 4096, policyCid),
    alice, 'taskMode.createTaskDefinition'
  );

  // Verify via RPC
  const taskDefs = await api.rpc.taskMode.listTaskDefinitions();
  console.log('  taskMode definitions:', taskDefs.toJSON());

  // === 2. TaskMode: create_task_order ===
  await submitAndWait(
    api.tx.taskMode.createTaskOrder(0, bob.address, 100, 50),
    alice, 'taskMode.createTaskOrder'
  );

  // === 3. ComputePoolScheduler: register_pool ===
  console.log('');
  console.log('--- Test 2: ComputePoolScheduler ---');
  const gpuModel = '0x' + Buffer.from('RTX4090').toString('hex');
  await submitAndWait(
    api.tx.computePoolScheduler.registerPool(gpuModel, 24576, false, 100, 1000000000000),
    alice, 'computePoolScheduler.registerPool'
  );

  // Verify via RPC
  const pool = await api.rpc.poolScheduler.getPool(0);
  console.log('  pool 0:', pool.toJSON());

  // === 4. ComputePoolScheduler: submit_task ===
  await submitAndWait(
    api.tx.computePoolScheduler.submitTask({ m: 128, n: 128, k: 128 }, 'Normal', null),
    bob, 'computePoolScheduler.submitTask'
  );

  // === 5. AgentAttestation: register_node ===
  console.log('');
  console.log('--- Test 3: AgentAttestation ---');
  const gpuUuid = '0x' + Buffer.from('GPU-uuid-test-1234').toString('hex');
  await submitAndWait(
    api.tx.agentAttestation.registerNode(gpuUuid, 100),
    alice, 'agentAttestation.registerNode'
  );

  // Verify via RPC
  const node = await api.rpc.dbc3.getNodeRegistration(alice.address);
  console.log('  node registration:', node.toJSON());

  // === 6. AgentAttestation: heartbeat ===
  await submitAndWait(
    api.tx.agentAttestation.heartbeat(),
    alice, 'agentAttestation.heartbeat'
  );

  // === 7. ZkCompute: submit_proof ===
  console.log('');
  console.log('--- Test 4: ZkCompute ---');
  const proof = '0x' + Buffer.from('zk-proof-data-test').toString('hex');
  await submitAndWait(
    api.tx.zkCompute.submitProof(proof, [128, 128, 128], 130, 1),
    alice, 'zkCompute.submitProof'
  );

  // === 8. X402Settlement: submit_payment_intent ===
  console.log('');
  console.log('--- Test 5: X402Settlement ---');
  const fakeSig = '0x' + '00'.repeat(32);
  const fingerprint = '0x' + '01'.repeat(32);
  await submitAndWait(
    api.tx.x402Settlement.submitPaymentIntent(bob.address, 500000000000, 1, fingerprint, fakeSig),
    alice, 'x402Settlement.submitPaymentIntent'
  );

  // Verify via RPC
  const intent = await api.rpc.x402.getPaymentIntent(0);
  console.log('  payment intent 0:', intent.toJSON());

  console.log('');
  console.log('=== All extrinsic tests completed ===');

} catch (e) {
  console.error('Error:', e.message);
}

await api.disconnect();
process.exit(0);
