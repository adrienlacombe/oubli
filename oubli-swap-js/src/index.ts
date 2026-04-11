// Main entry point for the Oubli Swap JS bundle.
// This runs inside QuickJS, embedded in the oubli-swap Rust crate.

import "./polyfills";
import { OubliStarknetAccount } from "./signer";
import { oubliStorageCtor } from "./storage";
import {
  SwapperFactory,
  type TypedSwapperOptions,
} from "@atomiqlabs/sdk";
import {
  StarknetInitializer,
  StarknetAssets,
  type StarknetOptions,
} from "@atomiqlabs/chain-starknet";
import { StarknetSigner } from "@atomiqlabs/chain-starknet";
import { Provider, constants, RpcProvider } from "starknet";
import { BitcoinNetwork } from "@atomiqlabs/base";

// Host functions
declare function __oubli_starknet_rpc_url(): string;
declare function __oubli_starknet_chain_id(): string;
declare function __oubli_log(level: string, message: string): void;
declare function __oubli_set_timeout(ms: number): Promise<void>;

// Types exposed to Rust via JSON serialization
export interface SwapQuote {
  swapId: string;
  inputAmount: string;
  outputAmount: string;
  fee: string;
  expiry: number;
  btcAddress?: string;  // For BTC→WBTC: where to send BTC
  lnInvoice?: string;   // For BTCLN→WBTC: lightning invoice
}

export interface SwapStatus {
  swapId: string;
  state: "created" | "btc_pending" | "btc_confirmed" | "claiming" | "completed" | "failed" | "refundable";
  txId?: string;
  message?: string;
}

// Global state
let swapperFactory: SwapperFactory<readonly [typeof StarknetInitializer]> | null = null;
let swapper: any = null;
let signerWrapper: any = null;

// Store created swap objects so executeSwap can find them
const activeSwaps: Map<string, any> = new Map();

async function getTrackedSwap(swapId: string): Promise<any | null> {
  const cached = activeSwaps.get(swapId);
  if (cached) {
    return cached;
  }
  if (!signerWrapper) {
    return null;
  }
  const loaded = await signerWrapper.getSwapById(swapId);
  if (loaded) {
    activeSwaps.set(swapId, loaded);
  }
  return loaded ?? null;
}

// Debug: collect fetch logs during init
let fetchLogs: string[] = [];
let collectFetchLogs = false;

const _origFetch = globalThis.fetch;
(globalThis as any).fetch = async function(input: RequestInfo | URL, init?: RequestInit) {
  const url = typeof input === 'string' ? input : (input as any)?.url ?? '?';
  const method = init?.method ?? 'GET';
  if (collectFetchLogs) {
    fetchLogs.push(`${method} ${url.substring(0, 80)}`);
  }
  try {
    const resp = await _origFetch.call(globalThis, input, init);
    if (collectFetchLogs) {
      const ct = resp.headers.get('content-type') ?? 'no-ct';
      const cloned = resp.clone();
      const buf = await cloned.arrayBuffer();
      const bytes = new Uint8Array(buf);
      const first20 = Array.from(bytes.slice(0, 20)).map((b: number) => b.toString(16).padStart(2, '0')).join(' ');
      fetchLogs.push(`  -> ${resp.status} ct=${ct} ${bytes.length}B [${first20}]`);
    }
    return resp;
  } catch (e: any) {
    if (collectFetchLogs) {
      fetchLogs.push(`  -> ERROR: ${e.message ?? e}`);
    }
    throw e;
  }
};

/**
 * Initialize the Atomiq swapper.
 * Called from Rust after host functions are registered.
 */
async function init(): Promise<string> {
  let step = "start";
  try {
    step = "1-config";
    const rpcUrl = __oubli_starknet_rpc_url();
    const chainIdStr = __oubli_starknet_chain_id();
    const isMainnet = chainIdStr === "SN_MAIN";
    const starknetChainId = isMainnet
      ? constants.StarknetChainId.SN_MAIN
      : constants.StarknetChainId.SN_SEPOLIA;

    step = "2-provider";
    const provider = new RpcProvider({ nodeUrl: rpcUrl });

    step = "3-factory";
    swapperFactory = new SwapperFactory([StarknetInitializer] as const);

    step = "4-options";
    const starknetOptions: StarknetOptions = {
      rpcUrl: provider,
      chainId: starknetChainId,
    };

    const options: TypedSwapperOptions<readonly [typeof StarknetInitializer]> = {
      chains: {
        STARKNET: starknetOptions,
      },
      bitcoinNetwork: isMainnet ? BitcoinNetwork.MAINNET : BitcoinNetwork.TESTNET,
      chainStorageCtor: oubliStorageCtor,
    };

    step = "5-newSwapper";
    swapper = swapperFactory.newSwapper(options);

    step = "6-swapper.init";
    collectFetchLogs = true;
    fetchLogs = [];
    await swapper.init();
    collectFetchLogs = false;

    // Diagnostic: check how many intermediaries were found
    try {
      const discovery = (swapper as any).intermediaryDiscovery ?? (swapper as any)._intermediaryDiscovery;
      if (discovery) {
        const intermediaries = discovery.intermediaries ?? discovery._intermediaries ?? [];
        __oubli_log("info", `Intermediaries found: ${intermediaries.length}`);
        for (const lp of intermediaries) {
          const url = lp.url ?? "unknown";
          const services = lp.services ? Object.keys(lp.services) : [];
          __oubli_log("info", `  LP: ${url}, services: ${services.join(", ")}`);
        }
      } else {
        // Try to find it by walking the object
        const keys = Object.keys(swapper).filter(k => k.toLowerCase().includes("intermed") || k.toLowerCase().includes("discov"));
        __oubli_log("info", `Swapper keys with intermediary/discovery: ${keys.join(", ") || "none"}`);
        __oubli_log("info", `Swapper keys: ${Object.keys(swapper).join(", ")}`);
      }
    } catch (diagErr: any) {
      __oubli_log("warn", `Diagnostic failed: ${diagErr.message}`);
    }

    step = "7-account";
    const account = new OubliStarknetAccount(provider);
    const starknetSigner = new StarknetSigner(account);

    (starknetSigner as any).signTransaction = undefined;

    step = "8-withSigner";
    signerWrapper = swapper.withChain("STARKNET").withSigner(starknetSigner);

    // Count intermediaries for diagnostic
    let lpCount = -1;
    try {
      const disc = (swapper as any).intermediaryDiscovery ?? (swapper as any)._intermediaryDiscovery;
      if (disc) {
        const lps = disc.intermediaries ?? disc._intermediaries ?? [];
        lpCount = lps.length;
      }
    } catch {}

    return JSON.stringify({ ok: true, lpCount, fetchLogs: fetchLogs.slice(0, 20) });
  } catch (e: any) {
    const msg = e.message ?? String(e);
    const stack = e.stack ?? '';
    return JSON.stringify({ ok: false, error: `[step:${step}] ${msg} | stack: ${stack}` });
  }
}

/**
 * Create a BTC → WBTC swap (on-ramp).
 * Returns a quote with the BTC address to send to.
 */
async function createBtcToWbtcSwap(amountSats: string, exactIn: boolean): Promise<string> {
  try {
    if (!signerWrapper || !swapperFactory) {
      return JSON.stringify({ ok: false, error: "Swapper not initialized" });
    }

    const tokens = swapperFactory.Tokens;
    const btcToken = tokens.BITCOIN.BTC;
    const wbtcToken = tokens.STARKNET.WBTC;

    const swap = await signerWrapper.create(
      btcToken,
      wbtcToken,
      BigInt(amountSats),
      exactIn,
    );
    activeSwaps.set(swap.getId(), swap);

    const quote: SwapQuote = {
      swapId: swap.getId(),
      inputAmount: swap.getInput().amount.toString(),
      outputAmount: swap.getOutput().amount.toString(),
      fee: swap.getFee?.().totalInSrcToken?.amount?.toString() ?? "0",
      expiry: swap.getExpiry?.() ?? 0,
      btcAddress: swap.getBitcoinAddress?.() ?? undefined,
    };

    __oubli_log("info", `BTC→WBTC swap created: ${quote.swapId}`);
    return JSON.stringify({ ok: true, quote });
  } catch (e: any) {
    __oubli_log("error", `createBtcToWbtcSwap failed: ${e.message ?? e}`);
    return JSON.stringify({ ok: false, error: e.message ?? String(e) });
  }
}

/**
 * Create a WBTC → BTC swap (off-ramp).
 * Locks WBTC in escrow, LP sends BTC.
 */
async function createWbtcToBtcSwap(
  amountSats: string,
  btcAddress: string,
  exactIn: boolean,
): Promise<string> {
  try {
    if (!signerWrapper || !swapperFactory) {
      return JSON.stringify({ ok: false, error: "Swapper not initialized" });
    }

    const tokens = swapperFactory.Tokens;
    const wbtcToken = tokens.STARKNET.WBTC;
    const btcToken = tokens.BITCOIN.BTC;

    const swap = await signerWrapper.create(
      wbtcToken,
      btcToken,
      BigInt(amountSats),
      exactIn,
      btcAddress,
    );
    activeSwaps.set(swap.getId(), swap);

    const quote: SwapQuote = {
      swapId: swap.getId(),
      inputAmount: swap.getInput().amount.toString(),
      outputAmount: swap.getOutput().amount.toString(),
      fee: swap.getFee?.().totalInSrcToken?.amount?.toString() ?? "0",
      expiry: swap.getExpiry?.() ?? 0,
    };

    __oubli_log("info", `WBTC→BTC swap created: ${quote.swapId}`);
    return JSON.stringify({ ok: true, quote });
  } catch (e: any) {
    __oubli_log("error", `createWbtcToBtcSwap failed: ${e.message ?? e}`);
    return JSON.stringify({ ok: false, error: e.message ?? String(e) });
  }
}

/**
 * Create a BTCLN → WBTC swap (Lightning on-ramp).
 * Returns a lightning invoice to pay.
 */
async function createLnToWbtcSwap(amountSats: string, exactIn: boolean): Promise<string> {
  try {
    if (!signerWrapper || !swapperFactory) {
      return JSON.stringify({ ok: false, error: "Swapper not initialized" });
    }

    const tokens = swapperFactory.Tokens;
    const btclnToken = tokens.BITCOIN.BTCLN;
    const wbtcToken = tokens.STARKNET.WBTC;

    const swap = await signerWrapper.create(
      btclnToken,
      wbtcToken,
      BigInt(amountSats),
      exactIn,
    );

    const swapId = swap.getId();
    activeSwaps.set(swapId, swap);

    const rawExpiry = swap.getExpiry?.();
    const lnInvoice = swap.getLightningInvoice?.() ?? swap.getAddress?.() ?? undefined;

    // If the SDK doesn't provide an expiry, parse it from the BOLT11 invoice.
    // BOLT11 invoices have a default expiry of 3600s if not specified.
    let expiry = typeof rawExpiry === "number" && rawExpiry > 0 ? rawExpiry : 0;
    if (expiry === 0 && lnInvoice) {
      // Estimate expiry: current time + 10 minutes (LP typical window)
      expiry = Math.floor(Date.now() / 1000) + 600;
    }

    const quote: SwapQuote = {
      swapId,
      inputAmount: swap.getInput().amount.toString(),
      outputAmount: swap.getOutput().amount.toString(),
      fee: swap.getFee?.().totalInSrcToken?.amount?.toString() ?? "0",
      expiry,
      lnInvoice,
    };

    __oubli_log("info", `BTCLN→WBTC swap created: ${quote.swapId}, rawExpiry=${rawExpiry}, expiry=${expiry}, invoice=${quote.lnInvoice?.substring(0, 30)}...`);
    return JSON.stringify({ ok: true, quote });
  } catch (e: any) {
    __oubli_log("error", `createLnToWbtcSwap failed: ${e.message ?? e}`);
    return JSON.stringify({ ok: false, error: e.message ?? String(e) });
  }
}

/**
 * Create a WBTC → BTCLN swap (pay a Lightning invoice).
 * User locks WBTC in escrow, LP pays the Lightning invoice.
 *
 * @param bolt11 - The BOLT11 Lightning invoice to pay.
 * @returns Quote with input_amount (WBTC needed) and output_amount (sats paid via LN).
 */
async function createWbtcToBtcLnSwap(bolt11: string): Promise<string> {
  try {
    if (!signerWrapper || !swapperFactory) {
      return JSON.stringify({ ok: false, error: "Swapper not initialized" });
    }

    // Parse the amount from the BOLT11 invoice
    const amountSats = parseBolt11Amount(bolt11);
    if (amountSats <= 0) {
      return JSON.stringify({ ok: false, error: "Invoice has no amount or amount is zero" });
    }

    __oubli_log("info", `createWbtcToBtcLnSwap: amount=${amountSats} sats, invoice=${bolt11.substring(0, 30)}...`);

    const tokens = swapperFactory.Tokens;
    const wbtcToken = tokens.STARKNET.WBTC;
    const btclnToken = tokens.BITCOIN.BTCLN;

    __oubli_log("info", `WBTC token: ${JSON.stringify(wbtcToken)}`);
    __oubli_log("info", `BTCLN token: ${JSON.stringify(btclnToken)}`);

    // Enable fetch logging during swap creation for diagnostics
    collectFetchLogs = true;
    fetchLogs = [];

    // exactIn=false: the output amount (LN payment) should be exactly the invoice amount.
    // The SDK calculates the required WBTC input (including fees).
    let swap;
    try {
      swap = await signerWrapper.create(
        wbtcToken,
        btclnToken,
        BigInt(amountSats),
        false, // exact output
        bolt11,
      );
    } finally {
      collectFetchLogs = false;
    }

    const swapId = swap.getId();
    activeSwaps.set(swapId, swap);

    const quote: SwapQuote = {
      swapId,
      inputAmount: swap.getInput().amount.toString(),
      outputAmount: swap.getOutput().amount.toString(),
      fee: swap.getFee?.().totalInSrcToken?.amount?.toString() ?? "0",
      expiry: swap.getExpiry?.() ?? 0,
      lnInvoice: bolt11,
    };

    __oubli_log(
      "info",
      `WBTC→BTCLN swap created: ${quote.swapId}, input=${quote.inputAmount}, output=${quote.outputAmount}`,
    );
    return JSON.stringify({ ok: true, quote });
  } catch (e: any) {
    // Add LP diagnostic + fetch logs to error
    let lpInfo = "";
    try {
      const disc = (swapper as any).intermediaryDiscovery;
      const lps = disc?.intermediaries ?? [];
      lpInfo = ` [LPs: ${lps.length}`;
      for (const lp of lps) {
        const url = lp.url ?? "?";
        const chains = Object.keys(lp.services || {});
        lpInfo += `, ${url}(${chains.join(",")})`;
      }
      lpInfo += "]";
    } catch {}
    const fetchInfo = fetchLogs.length > 0 ? ` [fetch: ${fetchLogs.slice(0, 30).join(" | ")}]` : "";
    const msg = (e.message ?? String(e)) + lpInfo + fetchInfo;
    return JSON.stringify({ ok: false, error: msg });
  }
}

/**
 * Parse the amount (in satoshis) from a BOLT11 Lightning invoice.
 * Returns 0 if the invoice has no amount.
 */
function parseBolt11Amount(invoice: string): number {
  const lower = invoice.toLowerCase();

  // Strip the prefix: lnbc (mainnet), lntb (testnet), lnbcrt (regtest)
  let rest: string;
  if (lower.startsWith("lnbcrt")) {
    rest = lower.slice(6);
  } else if (lower.startsWith("lnbc")) {
    rest = lower.slice(4);
  } else if (lower.startsWith("lntb")) {
    rest = lower.slice(4);
  } else {
    throw new Error("Not a valid BOLT11 invoice");
  }

  // Amount is digits + optional multiplier before the separator '1'
  const match = rest.match(/^(\d+)([munp]?)1/);
  if (!match) {
    // No amount in invoice (zero-amount invoice)
    return 0;
  }

  const amount = parseInt(match[1], 10);
  const multiplier = match[2];

  // Convert to BTC, then to sats
  let btcValue: number;
  switch (multiplier) {
    case "m":
      btcValue = amount * 0.001;
      break;
    case "u":
      btcValue = amount * 0.000001;
      break;
    case "n":
      btcValue = amount * 0.000000001;
      break;
    case "p":
      btcValue = amount * 0.000000000001;
      break;
    default:
      btcValue = amount;
      break;
  }

  return Math.round(btcValue * 100000000);
}

/**
 * Commit a swap (lock WBTC in escrow on Starknet) and wait for LP payment.
 *
 * For ToBTCLN swaps:
 *   1. commit() — sends Starknet TX to lock WBTC in escrow
 *   2. waitForPayment() — polls LP until Lightning invoice is paid
 */
async function executeSwap(swapId: string): Promise<string> {
  try {
    if (!signerWrapper) {
      return JSON.stringify({ ok: false, error: "Swapper not initialized" });
    }

    const swap = await getTrackedSwap(swapId);
    if (!swap) {
      return JSON.stringify({ ok: false, error: `Swap ${swapId} not found` });
    }

    __oubli_log("info", `executeSwap: state=${swap.getState?.()}, type=${swap.constructor?.name}`);

    // Step 1: Commit (lock WBTC in escrow)
    if (typeof swap.commit === "function") {
      __oubli_log("info", `executeSwap: committing escrow...`);
      const commitTxId = await swap.commit();
      __oubli_log("info", `executeSwap: committed, txId=${commitTxId}`);
    } else {
      __oubli_log("warn", `executeSwap: no commit() method, trying generic actions`);
      const actions = swap.getActions?.() ?? [];
      for (const action of actions) {
        if (typeof action.execute === "function") {
          await action.execute();
        }
      }
    }

    // Step 2: Wait for LP to pay the Lightning invoice (up to 2 min)
    if (typeof swap.waitForPayment === "function") {
      __oubli_log("info", `executeSwap: waiting for LP payment...`);
      const success = await swap.waitForPayment(120, 5);
      if (success) {
        __oubli_log("info", `executeSwap: payment confirmed!`);
        activeSwaps.delete(swapId);
        return JSON.stringify({ ok: true });
      } else {
        __oubli_log("error", `executeSwap: payment failed, swap may be refundable`);
        return JSON.stringify({ ok: false, error: "LP failed to pay Lightning invoice. Swap is refundable." });
      }
    }

    return JSON.stringify({ ok: true });
  } catch (e: any) {
    __oubli_log("error", `executeSwap failed: ${e.message ?? e}`);
    return JSON.stringify({ ok: false, error: e.message ?? String(e) });
  }
}

/**
 * Wait for an incoming Lightning payment and claim WBTC.
 * Used for BTCLN → WBTC swaps (receive Lightning).
 *
 * We do NOT use execute() because it tries auto-settlement via Nostr watchtowers
 * which requires WebSocket connections we can't support in QuickJS.
 *
 * Instead we do the manual flow:
 *   1. waitForPayment() — polls LP until the LN invoice is paid
 *   2. claim() — submits on-chain transaction to claim WBTC from escrow
 *
 * NOTE: waitForPayment signature is (onPaymentReceived?, checkIntervalSeconds?, abortSignal?)
 *       — first param is a CALLBACK, not a timeout!
 */
async function waitForIncomingSwap(swapId: string): Promise<string> {
  try {
    const swap = await getTrackedSwap(swapId);
    if (!swap) {
      return JSON.stringify({ ok: false, error: `Swap ${swapId} not found` });
    }

    __oubli_log("info", `waitForIncomingSwap: type=${swap.constructor?.name}, state=${swap.getState?.()}`);

    // Step 1: Wait for the payer to pay the LN invoice
    if (typeof swap.waitForPayment === "function") {
      __oubli_log("info", `waitForIncomingSwap: calling waitForPayment...`);
      const success = await swap.waitForPayment(
        (txId: string) => __oubli_log("info", `waitForIncomingSwap: payment received, txId=${txId}`),
        5, // poll every 5 seconds
      );
      __oubli_log("info", `waitForIncomingSwap: waitForPayment returned: ${success}, state=${swap.getState?.()}`);
      if (!success) {
        activeSwaps.delete(swapId);
        return JSON.stringify({ ok: false, error: "Payment not received within timeout. The invoice has expired." });
      }
    } else {
      __oubli_log("warn", `waitForIncomingSwap: no waitForPayment method!`);
    }

    // Step 2: Manual claim.
    // Auto-settlement (watchtower) requires Nostr WebSocket which doesn't work in QuickJS.
    // Per SDK docs: waitTillClaimed → if not settled → claim(signer).
    // We skip waitTillClaimed (it hangs due to setTimeout issues in rquickjs) and
    // go straight to manual claim after waiting for the LP's commit tx to be mined.
    const signer = (signerWrapper as any)?._signer ?? (signerWrapper as any)?.signer;
    __oubli_log("info", `waitForIncomingSwap: state=${swap.getState?.()}, hasSigner=${!!signer}`);

    if (typeof swap.claim === "function" && signer) {
      // Wait for LP's commit tx to be mined on Starknet (~6s block time).
      // Use __oubli_set_timeout directly (Rust tokio::sleep) to avoid JS setTimeout issues.
      __oubli_log("info", `waitForIncomingSwap: waiting 10s for LP commit to be mined...`);
      await __oubli_set_timeout(10000);
      __oubli_log("info", `waitForIncomingSwap: delay done, claiming WBTC on-chain...`);

      const maxAttempts = 4;
      for (let attempt = 1; attempt <= maxAttempts; attempt++) {
        try {
          __oubli_log("info", `waitForIncomingSwap: claim attempt ${attempt}/${maxAttempts}...`);
          const claimTxId = await swap.claim(signer);
          __oubli_log("info", `waitForIncomingSwap: claim tx submitted: ${claimTxId}`);
          break;
        } catch (claimErr: any) {
          const msg = claimErr.message ?? String(claimErr);
          if (attempt < maxAttempts && (msg.includes("Not committed") || msg.includes("not committed"))) {
            __oubli_log("info", `waitForIncomingSwap: commit not on-chain yet, waiting 15s more...`);
            await __oubli_set_timeout(15000);
            continue;
          }
          throw claimErr;
        }
      }
    } else {
      __oubli_log("warn", `waitForIncomingSwap: cannot claim — no signer or no claim method`);
    }

    activeSwaps.delete(swapId);
    return JSON.stringify({ ok: true });
  } catch (e: any) {
    const stack = e.stack ?? "";
    __oubli_log("error", `waitForIncomingSwap failed: ${e.message ?? e} | stack: ${stack}`);
    return JSON.stringify({ ok: false, error: e.message ?? String(e) });
  }
}

/**
 * Get the status of a swap.
 */
async function getSwapStatus(swapId: string): Promise<string> {
  try {
    if (!signerWrapper) {
      return JSON.stringify({ ok: false, error: "Swapper not initialized" });
    }

    const swap = await getTrackedSwap(swapId);
    if (!swap) {
      return JSON.stringify({ ok: false, error: `Swap ${swapId} not found` });
    }

    const status: SwapStatus = {
      swapId,
      state: mapSwapState(swap),
      txId: swap.getTxId?.() ?? undefined,
    };

    return JSON.stringify({ ok: true, status });
  } catch (e: any) {
    return JSON.stringify({ ok: false, error: e.message ?? String(e) });
  }
}

/**
 * Get all active/pending swaps.
 */
async function getAllSwaps(): Promise<string> {
  try {
    if (!signerWrapper) {
      return JSON.stringify({ ok: false, error: "Swapper not initialized" });
    }

    const swaps = await signerWrapper.getAllSwaps();
    for (const swap of swaps) {
      activeSwaps.set(swap.getId(), swap);
    }
    const result = swaps.map((s: any) => ({
      swapId: s.getId(),
      state: mapSwapState(s),
      inputAmount: s.getInput().amount.toString(),
      outputAmount: s.getOutput().amount.toString(),
    }));

    return JSON.stringify({ ok: true, swaps: result });
  } catch (e: any) {
    return JSON.stringify({ ok: false, error: e.message ?? String(e) });
  }
}

/**
 * Get swap limits for BTC↔WBTC.
 */
async function getSwapLimits(direction: "btc_to_wbtc" | "wbtc_to_btc"): Promise<string> {
  try {
    if (!signerWrapper || !swapperFactory) {
      return JSON.stringify({ ok: false, error: "Swapper not initialized" });
    }

    const tokens = swapperFactory.Tokens;
    let limits;
    if (direction === "btc_to_wbtc") {
      limits = signerWrapper.getSwapLimits(tokens.BITCOIN.BTC, tokens.STARKNET.WBTC);
    } else {
      limits = signerWrapper.getSwapLimits(tokens.STARKNET.WBTC, tokens.BITCOIN.BTC);
    }

    return JSON.stringify({
      ok: true,
      limits: {
        input: {
          min: limits.input.min.amount.toString(),
          max: limits.input.max?.amount?.toString() ?? null,
        },
        output: {
          min: limits.output.min.amount.toString(),
          max: limits.output.max?.amount?.toString() ?? null,
        },
      },
    });
  } catch (e: any) {
    return JSON.stringify({ ok: false, error: e.message ?? String(e) });
  }
}

function mapSwapState(swap: any): SwapStatus["state"] {
  // Map internal swap states to our simplified state enum
  const state = swap.getState?.();
  if (state === undefined || state === null) return "created";

  // Common state names across swap types
  const stateStr = String(state);
  if (stateStr.includes("REFUND") || stateStr.includes("refund")) return "refundable";
  if (stateStr.includes("CLAIM") || stateStr.includes("claim")) return "claiming";
  if (stateStr.includes("COMPLETE") || stateStr.includes("SUCCESS") || stateStr.includes("success")) return "completed";
  if (stateStr.includes("FAIL") || stateStr.includes("fail")) return "failed";
  if (stateStr.includes("BTC_TX") || stateStr.includes("btc")) return "btc_pending";

  return "created";
}

// Expose functions to Rust host
(globalThis as any).__oubli_swap = {
  init,
  createBtcToWbtcSwap,
  createWbtcToBtcSwap,
  createLnToWbtcSwap,
  createWbtcToBtcLnSwap,
  executeSwap,
  waitForIncomingSwap,
  getSwapStatus,
  getAllSwaps,
  getSwapLimits,
};
