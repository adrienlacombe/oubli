// Polyfills for QuickJS environment.
// These bridge to Rust host functions provided by oubli-swap crate.

// __oubli_fetch(url, method, headers_json, body) → {status, headers_json, body}
// __oubli_setTimeout(callback_id, ms)
// __oubli_log(level, message)

declare function __oubli_fetch(
  url: string,
  method: string,
  headersJson: string,
  body: string,
): Promise<{ status: string; headers: string; body: string }>;

declare function __oubli_set_timeout(ms: number): Promise<void>;

declare function __oubli_log(level: string, message: string): void;

// Polyfill fetch
if (typeof globalThis.fetch === "undefined") {
  (globalThis as any).fetch = async (
    input: string | Request,
    init?: RequestInit,
  ): Promise<Response> => {
    const url = typeof input === "string" ? input : input.url;
    const method = init?.method ?? "GET";
    const headers: Record<string, string> = {};
    if (init?.headers) {
      if (init.headers instanceof Headers) {
        init.headers.forEach((v, k) => (headers[k] = v));
      } else if (Array.isArray(init.headers)) {
        for (const [k, v] of init.headers) headers[k] = v;
      } else {
        Object.assign(headers, init.headers);
      }
    }
    const body =
      typeof init?.body === "string" ? init.body : init?.body?.toString() ?? "";

    const result = await __oubli_fetch(
      url,
      method,
      JSON.stringify(headers),
      body,
    );

    const responseHeaders = new Headers(JSON.parse(result.headers));
    return new Response(result.body, {
      status: parseInt(result.status, 10),
      headers: responseHeaders,
    });
  };
}

// Polyfill setTimeout using the host sleep function
if (typeof globalThis.setTimeout === "undefined") {
  let nextId = 1;
  const callbacks = new Map<number, () => void>();

  (globalThis as any).setTimeout = (fn: () => void, ms?: number): number => {
    const id = nextId++;
    __oubli_set_timeout(ms ?? 0).then(() => {
      const cb = callbacks.get(id);
      if (cb) {
        callbacks.delete(id);
        cb();
      }
    });
    callbacks.set(id, fn);
    return id;
  };

  (globalThis as any).clearTimeout = (id: number): void => {
    callbacks.delete(id);
  };
}

// Polyfill setInterval
if (typeof globalThis.setInterval === "undefined") {
  (globalThis as any).setInterval = (fn: () => void, ms: number): number => {
    const interval = () => {
      fn();
      (globalThis as any).setTimeout(interval, ms);
    };
    return (globalThis as any).setTimeout(interval, ms);
  };

  (globalThis as any).clearInterval = (id: number): void => {
    (globalThis as any).clearTimeout(id);
  };
}

// Console polyfill
if (typeof globalThis.console === "undefined") {
  (globalThis as any).console = {
    log: (...args: any[]) => __oubli_log("info", args.map(String).join(" ")),
    info: (...args: any[]) => __oubli_log("info", args.map(String).join(" ")),
    warn: (...args: any[]) => __oubli_log("warn", args.map(String).join(" ")),
    error: (...args: any[]) =>
      __oubli_log("error", args.map(String).join(" ")),
    debug: (...args: any[]) =>
      __oubli_log("debug", args.map(String).join(" ")),
  };
}

export {};
