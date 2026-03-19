// Custom Starknet Account that routes all transactions through the AVNU paymaster.
// Private keys stay in Rust; signing is delegated via host functions.

import {
  Account,
  Provider,
  CallData,
  hash,
  ec,
  constants,
  typedData as typedDataModule,
  type Call,
  type InvokeFunctionResponse,
  type UniversalDetails,
  type AllowArray,
} from "starknet";

// Host functions provided by Rust
declare function __oubli_starknet_address(): string;
declare function __oubli_starknet_public_key(): string;
declare function __oubli_starknet_sign(hash: string): Promise<{ r: string; s: string }>;
declare function __oubli_starknet_chain_id(): string;
declare function __oubli_account_class_hash(): string;
declare function __oubli_paymaster_url(): string;
declare function __oubli_paymaster_api_key(): string;
declare function __oubli_log(level: string, message: string): void;

let paymasterRequestId = 1;

/**
 * Issue a JSON-RPC 2.0 call to the AVNU paymaster.
 */
async function paymasterRpc(method: string, params: any): Promise<any> {
  const url = __oubli_paymaster_url();
  const apiKey = __oubli_paymaster_api_key();

  const body = JSON.stringify({
    jsonrpc: "2.0",
    id: paymasterRequestId++,
    method,
    params,
  });

  const headers: Record<string, string> = {
    "Content-Type": "application/json",
  };
  if (apiKey) {
    headers["x-paymaster-api-key"] = apiKey;
  }

  const resp = await fetch(url, {
    method: "POST",
    headers,
    body,
  });

  const json = await resp.json();

  if (json.error) {
    const code = json.error.code ?? 0;
    const msg = json.error.message ?? "unknown";
    const data = json.error.data ? JSON.stringify(json.error.data) : "";
    throw new Error(`Paymaster ${method}: code=${code}, message="${msg}" ${data}`);
  }

  return json.result;
}

/**
 * Build the fee_mode / user_parameters for SNIP-29.
 */
function userParameters(): any {
  const apiKey = __oubli_paymaster_api_key();
  const feeMode = apiKey
    ? { mode: "sponsored" }
    : {
        mode: "default",
        gas_token:
          "0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d",
      };
  return { version: "0x1", fee_mode: feeMode };
}

/**
 * Convert starknet.js Call to SNIP-29 paymaster call format.
 */
function callToPaymaster(call: Call): any {
  // entrypoint may be a name ("transfer") or a selector ("0x...")
  const selector = call.entrypoint.startsWith("0x")
    ? call.entrypoint
    : hash.getSelectorFromName(call.entrypoint);

  // Normalize calldata to hex strings
  const calldata = ([...((call.calldata ?? []) as any[])] as any[]).map((v: any) => {
    if (typeof v === "string" && v.startsWith("0x")) return v;
    if (typeof v === "bigint") return "0x" + v.toString(16);
    return "0x" + BigInt(v).toString(16);
  });

  return {
    to: call.contractAddress,
    selector,
    calldata,
  };
}

/**
 * A starknet.js Account that routes all transactions through the AVNU paymaster.
 * Signing is delegated to Rust host functions (private keys never leave Rust).
 */
export class OubliStarknetAccount extends Account {
  private _publicKey: string;

  constructor(provider: Provider) {
    const address = __oubli_starknet_address();
    const publicKey = __oubli_starknet_public_key();

    // Dummy private key — we override signing and execution.
    super({
      provider,
      address,
      signer: ec.starkCurve.utils.randomPrivateKey(),
      cairoVersion: "1",
    });

    this._publicKey = publicKey;

    // Monkey-patch the internal signer's signRaw to delegate to Rust.
    // This is used for signMessage (e.g., HTLC claim signatures).
    const originalSigner = (this as any).signer;
    if (originalSigner) {
      originalSigner.signRaw = async (msgHash: string) => {
        const sig = await __oubli_starknet_sign(msgHash);
        return [sig.r, sig.s];
      };
    }
  }

  /**
   * Override execute() to route all transactions through the AVNU paymaster.
   * This replaces the default Account.execute() which submits directly via RPC.
   */
  async execute(
    calls: AllowArray<Call>,
    _details?: UniversalDetails,
  ): Promise<InvokeFunctionResponse> {
    const callArray = Array.isArray(calls) ? calls : [calls];
    const address = __oubli_starknet_address();

    __oubli_log("info", `Paymaster execute: ${callArray.length} call(s)`);

    // 1. Convert calls to paymaster format
    const paymasterCalls = callArray.map(callToPaymaster);

    // 2. Build typed data from paymaster
    const buildResult = await paymasterRpc("paymaster_buildTransaction", {
      transaction: {
        type: "invoke",
        invoke: {
          user_address: address,
          calls: paymasterCalls,
        },
      },
      parameters: userParameters(),
    });

    const td = buildResult.typed_data;
    if (!td) {
      throw new Error("Paymaster: missing typed_data in build response");
    }

    // 3. Compute SNIP-12 message hash
    const msgHash = typedDataModule.getMessageHash(td, address);

    // 4. Sign with Rust
    const sig = await __oubli_starknet_sign(msgHash);

    // 5. Execute via paymaster
    const execResult = await paymasterRpc("paymaster_executeTransaction", {
      transaction: {
        type: "invoke",
        invoke: {
          user_address: address,
          typed_data: td,
          signature: [sig.r, sig.s],
        },
      },
      parameters: userParameters(),
    });

    const txHash = execResult.transaction_hash;
    if (!txHash) {
      throw new Error("Paymaster: missing transaction_hash in execute response");
    }

    __oubli_log("info", `Paymaster tx submitted: ${txHash}`);
    return { transaction_hash: txHash };
  }

  /**
   * Return undefined — account deployment is handled by the paymaster.
   */
  getDeploymentData(): undefined {
    return undefined;
  }
}
