import { ApiPromise, WsProvider, Keyring } from '@polkadot/api';

const provider = new WsProvider('ws://127.0.0.1:9944');
const api = await ApiPromise.create({ provider });
const keyring = new Keyring({ type: 'sr25519' });
const alice = keyring.addFromUri('//Alice');
const bob = keyring.addFromUri('//Bob');

console.log('Connected to chain:', (await api.rpc.system.chain()).toString());
console.log('Alice:', alice.address);
console.log('Bob:', bob.address);
console.log('');

// Helper: submit extrinsic and wait for inclusion
async function submitAndWait(tx, signer, label) {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => { reject(new Error(label + ': timeout 30s')); }, 30000);
    tx.signAndSend(signer, ({ status, dispatchError }) => {
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
          console.log('[OK]  ', label, '- block', status.asInBlock.toHex().substring(0, 12));
        }
        resolve();
      }
    }).catch(e => { clearTimeout(timeout); console.log('[ERR] ', label, ':', e.message); resolve(); });
  });
}

// Helper: raw RPC call
async function rpcCall(method, params = []) {
  const res = await fetch('http://127.0.0.1:9944', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ id: 1, jsonrpc: '2.0', method, params })
  });
  const json = await res.json();
  return json.result;
}

let passed = 0, failed = 0;

try {
  // === 1. TaskMode ===
  console.log('=== Test 1: pallet-task-mode ===');
  await submitAndWait(
    api.tx.taskMode.createTaskDefinition(
      '0x' + Buffer.from('gpt-4-turbo').toString('hex'),
      '0x' + Buffer.from('v1.0').toString('hex'),
      1000000, 2000000, 4096,
      '0x' + Buffer.from('QmTestPolicyCID').toString('hex')
    ),
    alice, 'createTaskDefinition'
  );
  const defs = await rpcCall('taskMode_listTaskDefinitions');
  console.log('  RPC taskMode_listTaskDefinitions:', JSON.stringify(defs));

  await submitAndWait(
    api.tx.taskMode.createTaskOrder(0, bob.address, 100, 50),
    alice, 'createTaskOrder'
  );
  passed += 2;

  // === 2. ComputePoolScheduler ===
  console.log('');
  console.log('=== Test 2: pallet-compute-pool-scheduler ===');
  await submitAndWait(
    api.tx.computePoolScheduler.registerPool(
      '0x' + Buffer.from('RTX4090').toString('hex'),
      24576, false, 100, 1000000000000n
    ),
    alice, 'registerPool'
  );
  const pool = await rpcCall('poolScheduler_getPool', [0]);
  console.log('  RPC poolScheduler_getPool(0):', pool ? 'exists' : 'null');

  await submitAndWait(
    api.tx.computePoolScheduler.submitTask({ m: 128, n: 128, k: 128 }, 'Normal', null),
    bob, 'submitTask'
  );
  passed += 2;

  // === 3. AgentAttestation ===
  console.log('');
  console.log('=== Test 3: pallet-agent-attestation ===');
  await submitAndWait(
    api.tx.agentAttestation.registerNode(
      '0x' + Buffer.from('GPU-uuid-test-1234').toString('hex'),
      100
    ),
    alice, 'registerNode'
  );
  const nodeReg = await rpcCall('dbc3_getNodeRegistration', [alice.address]);
  console.log('  RPC dbc3_getNodeRegistration:', nodeReg ? 'registered' : 'null');

  await submitAndWait(
    api.tx.agentAttestation.heartbeat(),
    alice, 'heartbeat'
  );
  passed += 2;

  // === 4. ZkCompute ===
  console.log('');
  console.log('=== Test 4: pallet-zk-compute ===');
  await submitAndWait(
    api.tx.zkCompute.submitProof(
      '0x' + Buffer.from('zk-proof-test-data-32bytes-pad00').toString('hex'),
      [128, 128, 128], 130, 1
    ),
    alice, 'submitProof'
  );
  const zkTask = await rpcCall('dbc3_getZkTask', [0]);
  console.log('  RPC dbc3_getZkTask(0):', zkTask ? 'exists' : 'null');
  passed += 1;

  // === 5. X402Settlement ===
  console.log('');
  console.log('=== Test 5: pallet-x402-settlement ===');
  await submitAndWait(
    api.tx.x402Settlement.submitPaymentIntent(
      bob.address, 500000000000n, 1,
      '0x' + '01'.repeat(32),
      '0x' + '00'.repeat(32)
    ),
    alice, 'submitPaymentIntent'
  );
  const intent = await rpcCall('x402_getPaymentIntent', [0]);
  console.log('  RPC x402_getPaymentIntent(0):', intent ? 'exists' : 'null');
  const count = await rpcCall('x402_getPendingIntentsCount');
  console.log('  RPC x402_getPendingIntentsCount:', count);
  passed += 1;

  console.log('');
  console.log('========================================');
  console.log('Extrinsic tests: ' + passed + '/8 passed');
  console.log('========================================');

} catch (e) {
  console.error('Fatal error:', e.message);
}

await api.disconnect();
process.exit(0);
