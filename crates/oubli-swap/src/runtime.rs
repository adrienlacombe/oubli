//! QuickJS runtime that hosts the Atomiq SDK bundle.
//!
//! Provides Rust host functions to JS for: HTTP fetch, Starknet signing,
//! key-value storage, and timing.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use rquickjs::{
    async_with,
    function::{Async, Func},
    AsyncContext, AsyncRuntime, CatchResultExt, Function, Object, Value,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Collects fetch URLs for debugging LP discovery.
pub static FETCH_LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

use crate::error::SwapError;

/// Configuration for the JS runtime's Starknet connection.
pub struct RuntimeConfig {
    pub starknet_address: String,
    pub starknet_public_key: String,
    pub starknet_chain_id: String,
    pub starknet_rpc_url: String,
    pub account_class_hash: String,
    /// AVNU paymaster URL for gasless transaction submission.
    pub paymaster_url: String,
    /// Optional paymaster API key (enables sponsored mode).
    pub paymaster_api_key: Option<String>,
}

/// Callback trait for Starknet signing (implemented by WalletCore).
pub trait StarknetSignerCallback: Send + Sync + 'static {
    /// Sign a message hash with the Starknet private key.
    /// Returns (r, s) as hex strings.
    fn sign(&self, message_hash: &str) -> std::result::Result<(String, String), String>;
}

/// Simple key-value storage for swap state persistence.
pub trait SwapStorage: Send + Sync + 'static {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&self, key: &str, value: &str);
    fn remove(&self, key: &str);
}

/// In-memory swap storage (default).
#[derive(Default)]
pub struct InMemorySwapStorage {
    data: Mutex<HashMap<String, String>>,
}

impl SwapStorage for InMemorySwapStorage {
    fn get(&self, key: &str) -> Option<String> {
        self.data.lock().unwrap().get(key).cloned()
    }
    fn set(&self, key: &str, value: &str) {
        self.data
            .lock()
            .unwrap()
            .insert(key.to_string(), value.to_string());
    }
    fn remove(&self, key: &str) {
        self.data.lock().unwrap().remove(key);
    }
}

/// The embedded JS runtime.
pub struct JsRuntime {
    _rt: AsyncRuntime,
    ctx: AsyncContext,
}

impl JsRuntime {
    /// Create a new JS runtime with the Atomiq SDK bundle loaded.
    pub async fn new(
        config: RuntimeConfig,
        signer: Arc<dyn StarknetSignerCallback>,
        storage: Arc<dyn SwapStorage>,
    ) -> std::result::Result<Self, SwapError> {
        let rt = AsyncRuntime::new().map_err(|e| SwapError::Runtime(e.to_string()))?;

        let ctx = AsyncContext::full(&rt)
            .await
            .map_err(|e| SwapError::Runtime(e.to_string()))?;

        // Register host functions
        let config_arc = Arc::new(config);

        {
            let config_for_init = config_arc.clone();
            let signer_for_init = signer.clone();
            let storage_for_init = storage.clone();

            async_with!(ctx => |ctx| {
                let globals = ctx.globals();

                // ── Sync config getters ─────────────────────────────────
                let addr = config_for_init.starknet_address.clone();
                globals
                    .set("__oubli_starknet_address", Func::from(move || addr.clone()))
                    .unwrap();

                let pk = config_for_init.starknet_public_key.clone();
                globals
                    .set(
                        "__oubli_starknet_public_key",
                        Func::from(move || pk.clone()),
                    )
                    .unwrap();

                let cid = config_for_init.starknet_chain_id.clone();
                globals
                    .set(
                        "__oubli_starknet_chain_id",
                        Func::from(move || cid.clone()),
                    )
                    .unwrap();

                let rpc = config_for_init.starknet_rpc_url.clone();
                globals
                    .set(
                        "__oubli_starknet_rpc_url",
                        Func::from(move || rpc.clone()),
                    )
                    .unwrap();

                let ch = config_for_init.account_class_hash.clone();
                globals
                    .set(
                        "__oubli_account_class_hash",
                        Func::from(move || ch.clone()),
                    )
                    .unwrap();

                let pm_url = config_for_init.paymaster_url.clone();
                globals
                    .set(
                        "__oubli_paymaster_url",
                        Func::from(move || pm_url.clone()),
                    )
                    .unwrap();

                let pm_key = config_for_init
                    .paymaster_api_key
                    .clone()
                    .unwrap_or_default();
                globals
                    .set(
                        "__oubli_paymaster_api_key",
                        Func::from(move || pm_key.clone()),
                    )
                    .unwrap();

                // ── Async: Starknet signing ─────────────────────────────
                let signer_clone = signer_for_init.clone();
                globals
                    .set(
                        "__oubli_starknet_sign",
                        Func::from(Async(move |hash: String| {
                            let signer = signer_clone.clone();
                            async move {
                                match signer.sign(&hash) {
                                    Ok((r, s)) => {
                                        let mut m = HashMap::new();
                                        m.insert("r".to_string(), r);
                                        m.insert("s".to_string(), s);
                                        Ok::<_, rquickjs::Error>(m)
                                    }
                                    Err(e) => Err(rquickjs::Error::new_from_js_message(
                                        "sign", "failed", &e,
                                    )),
                                }
                            }
                        })),
                    )
                    .unwrap();

                // ── Async: HTTP fetch ───────────────────────────────────
                // Returns a JSON string: {"status":200,"headers":"{}","body":"..."}
                // Body param: empty string means no body, non-empty means send body.
                globals
                    .set(
                        "__oubli_fetch",
                        Func::from(Async(
                            move |url: String,
                                  method: String,
                                  headers_json: String,
                                  body: String| async move {
                                if let Ok(mut log) = FETCH_LOG.lock() {
                                    log.push(format!("{} {} -> ", method, &url[..url.len().min(80)]));
                                }
                                let client = reqwest::Client::builder()
                                    .user_agent("Oubli/1.0")
                                    .build()
                                    .unwrap_or_else(|_| reqwest::Client::new());
                                let headers: HashMap<String, String> =
                                    serde_json::from_str(&headers_json).unwrap_or_default();

                                let mut req = match method.to_uppercase().as_str() {
                                    "POST" => client.post(&url),
                                    "PUT" => client.put(&url),
                                    "DELETE" => client.delete(&url),
                                    "PATCH" => client.patch(&url),
                                    _ => client.get(&url),
                                };

                                for (k, v) in &headers {
                                    req = req.header(k.as_str(), v.as_str());
                                }

                                // Patch: starknet.js v6 uses "pre_confirmed" block tag
                                // which older RPC endpoints don't support.
                                let body = body.replace("\"pre_confirmed\"", "\"pending\"");

                                // Handle base64-encoded binary request bodies
                                if body.starts_with("__b64:") {
                                    if let Ok(bytes) = B64.decode(&body[6..]) {
                                        req = req.body(bytes);
                                    }
                                } else if !body.is_empty() {
                                    req = req.body(body);
                                }

                                match req.send().await {
                                    Ok(resp) => {
                                        let status = resp.status().as_u16();
                                        if let Ok(mut log) = FETCH_LOG.lock() {
                                            if let Some(last) = log.last_mut() {
                                                last.push_str(&format!("{}", status));
                                            }
                                        }
                                        let resp_headers: HashMap<String, String> = resp
                                            .headers()
                                            .iter()
                                            .filter_map(|(k, v)| {
                                                v.to_str()
                                                    .ok()
                                                    .map(|v| (k.to_string(), v.to_string()))
                                            })
                                            .collect();
                                        // Read as raw bytes to preserve binary data
                                        let body_bytes = resp.bytes().await.unwrap_or_default();
                                        let body_b64 = B64.encode(&body_bytes);
                                        let mut result = HashMap::new();
                                        result.insert("status".to_string(), status.to_string());
                                        result.insert(
                                            "headers".to_string(),
                                            serde_json::to_string(&resp_headers)
                                                .unwrap_or_default(),
                                        );
                                        result.insert("body_b64".to_string(), body_b64);
                                        Ok::<_, rquickjs::Error>(result)
                                    }
                                    Err(e) => {
                                        if let Ok(mut log) = FETCH_LOG.lock() {
                                            if let Some(last) = log.last_mut() {
                                                last.push_str(&format!("ERR:{}", e));
                                            }
                                        }
                                        Err(rquickjs::Error::new_from_js_message(
                                            "fetch",
                                            "network",
                                            &e.to_string(),
                                        ))
                                    },
                                }
                            },
                        )),
                    )
                    .unwrap();

                // ── Async: setTimeout ───────────────────────────────────
                globals
                    .set(
                        "__oubli_set_timeout",
                        Func::from(Async(move |ms: u64| async move {
                            tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                            Ok::<_, rquickjs::Error>(())
                        })),
                    )
                    .unwrap();

                // ── Sync: Storage ───────────────────────────────────────
                let sg = storage_for_init.clone();
                globals
                    .set(
                        "__oubli_storage_get",
                        Func::from(move |key: String| -> Option<String> { sg.get(&key) }),
                    )
                    .unwrap();

                let ss = storage_for_init.clone();
                globals
                    .set(
                        "__oubli_storage_set",
                        Func::from(move |key: String, value: String| {
                            ss.set(&key, &value);
                        }),
                    )
                    .unwrap();

                let sr = storage_for_init.clone();
                globals
                    .set(
                        "__oubli_storage_remove",
                        Func::from(move |key: String| {
                            sr.remove(&key);
                        }),
                    )
                    .unwrap();

                // ── Sync: Logging ───────────────────────────────────────
                globals
                    .set(
                        "__oubli_log",
                        Func::from(|level: String, message: String| {
                            crate::swap_debug_event!(
                                "swap.runtime.js",
                                "message",
                                "level" = level,
                                "message" = message
                            );
                        }),
                    )
                    .unwrap();

                // ── Polyfill: globalThis.crypto.getRandomValues ──────────
                // Required by @noble/hashes (browser path) for CSPRNG.
                globals
                    .set(
                        "__oubli_random_bytes",
                        Func::from(|n: usize| -> Vec<u8> {
                            let mut buf = vec![0u8; n];
                            getrandom::getrandom(&mut buf).expect("getrandom failed");
                            buf
                        }),
                    )
                    .unwrap();
            })
            .await;
        }

        let runtime = Self { _rt: rt, ctx };
        crate::swap_debug_event!("swap.runtime", "install_polyfills_started");
        runtime.install_polyfills().await?;
        crate::swap_debug_event!("swap.runtime", "polyfills_ready");
        runtime.load_bundle().await?;
        crate::swap_debug_event!("swap.runtime", "bundle_loaded");
        Ok(runtime)
    }

    /// Install browser API polyfills missing from QuickJS (crypto, TextEncoder, etc.).
    async fn install_polyfills(&self) -> std::result::Result<(), SwapError> {
        let polyfill = r#"
            // ── crypto.getRandomValues ──
            if (typeof globalThis.crypto === 'undefined') {
                globalThis.crypto = {};
            }
            if (typeof globalThis.crypto.getRandomValues !== 'function') {
                globalThis.crypto.getRandomValues = function(arr) {
                    const bytes = __oubli_random_bytes(arr.length);
                    for (let i = 0; i < arr.length; i++) {
                        arr[i] = bytes[i];
                    }
                    return arr;
                };
            }

            // ── TextEncoder / TextDecoder (UTF-8 only) ──
            if (typeof globalThis.TextEncoder === 'undefined') {
                globalThis.TextEncoder = class TextEncoder {
                    get encoding() { return 'utf-8'; }
                    encode(str) {
                        str = str === undefined ? '' : String(str);
                        const bytes = [];
                        for (let i = 0; i < str.length; i++) {
                            let c = str.charCodeAt(i);
                            if (c < 0x80) {
                                bytes.push(c);
                            } else if (c < 0x800) {
                                bytes.push(0xc0 | (c >> 6), 0x80 | (c & 0x3f));
                            } else if (c >= 0xd800 && c < 0xdc00) {
                                const next = str.charCodeAt(++i);
                                c = 0x10000 + ((c - 0xd800) << 10) + (next - 0xdc00);
                                bytes.push(
                                    0xf0 | (c >> 18),
                                    0x80 | ((c >> 12) & 0x3f),
                                    0x80 | ((c >> 6) & 0x3f),
                                    0x80 | (c & 0x3f)
                                );
                            } else {
                                bytes.push(0xe0 | (c >> 12), 0x80 | ((c >> 6) & 0x3f), 0x80 | (c & 0x3f));
                            }
                        }
                        return new Uint8Array(bytes);
                    }
                };
            }
            if (typeof globalThis.TextDecoder === 'undefined') {
                globalThis.TextDecoder = class TextDecoder {
                    constructor(label) { this._encoding = label || 'utf-8'; }
                    get encoding() { return this._encoding; }
                    decode(buf) {
                        const bytes = buf instanceof Uint8Array ? buf : new Uint8Array(buf);
                        const chars = [];
                        for (let i = 0; i < bytes.length; ) {
                            let c = bytes[i];
                            if (c < 0x80) {
                                chars.push(c); i++;
                            } else if ((c & 0xe0) === 0xc0) {
                                chars.push(((c & 0x1f) << 6) | (bytes[i+1] & 0x3f)); i += 2;
                            } else if ((c & 0xf0) === 0xe0) {
                                chars.push(((c & 0x0f) << 12) | ((bytes[i+1] & 0x3f) << 6) | (bytes[i+2] & 0x3f)); i += 3;
                            } else {
                                const cp = ((c & 0x07) << 18) | ((bytes[i+1] & 0x3f) << 12) | ((bytes[i+2] & 0x3f) << 6) | (bytes[i+3] & 0x3f);
                                const offset = cp - 0x10000;
                                chars.push(0xd800 + (offset >> 10), 0xdc00 + (offset & 0x3ff));
                                i += 4;
                            }
                        }
                        return String.fromCharCode(...chars);
                    }
                };
            }

            // ── window / navigator / location stubs ──
            if (typeof globalThis.window === 'undefined') {
                globalThis.window = globalThis;
            }
            if (typeof globalThis.navigator === 'undefined') {
                globalThis.navigator = { userAgent: 'QuickJS' };
            }
            if (typeof globalThis.location === 'undefined') {
                globalThis.location = { protocol: 'https:', hostname: 'localhost', href: 'https://localhost' };
            }

            // ── setTimeout / setInterval / clearTimeout / clearInterval ──
            if (typeof globalThis.setTimeout === 'undefined') {
                let _timerId = 0;
                const _timers = {};
                globalThis.setTimeout = function(fn, ms) {
                    const id = ++_timerId;
                    _timers[id] = true;
                    __oubli_set_timeout(ms || 0).then(() => {
                        if (_timers[id]) { delete _timers[id]; fn(); }
                    });
                    return id;
                };
                globalThis.clearTimeout = function(id) { delete _timers[id]; };
                globalThis.setInterval = function(fn, ms) {
                    const id = ++_timerId;
                    _timers[id] = true;
                    function tick() {
                        __oubli_set_timeout(ms || 0).then(() => {
                            if (_timers[id]) { fn(); tick(); }
                        });
                    }
                    tick();
                    return id;
                };
                globalThis.clearInterval = function(id) { delete _timers[id]; };
            }

            // ── URL class stub ──
            if (typeof globalThis.URL === 'undefined') {
                globalThis.URL = class URL {
                    constructor(url, base) {
                        if (base && !url.match(/^https?:\/\//)) {
                            url = base.replace(/\/$/, '') + '/' + url.replace(/^\//, '');
                        }
                        this.href = url;
                        const m = url.match(/^(https?:)\/\/([^/:]+)(:\d+)?(\/[^?#]*)?(\?[^#]*)?(#.*)?$/);
                        if (m) {
                            this.protocol = m[1];
                            this.hostname = m[2];
                            this.port = m[3] ? m[3].slice(1) : '';
                            this.pathname = m[4] || '/';
                            this.search = m[5] || '';
                            this.hash = m[6] || '';
                            this.host = this.hostname + (this.port ? ':' + this.port : '');
                            this.origin = this.protocol + '//' + this.host;
                        }
                        this.searchParams = { get: function(k) { return null; } };
                    }
                    toString() { return this.href; }
                };
            }

            // ── performance.now() ──
            if (typeof globalThis.performance === 'undefined') {
                const _start = Date.now();
                globalThis.performance = { now: function() { return Date.now() - _start; } };
            }

            // ── btoa / atob ──
            if (typeof globalThis.btoa === 'undefined') {
                const _chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=';
                globalThis.btoa = function(s) {
                    let r = '';
                    for (let i = 0; i < s.length; i += 3) {
                        const a = s.charCodeAt(i), b = s.charCodeAt(i+1), c = s.charCodeAt(i+2);
                        const bits = (a << 16) | ((b || 0) << 8) | (c || 0);
                        r += _chars[(bits >> 18) & 63] + _chars[(bits >> 12) & 63];
                        r += i+1 < s.length ? _chars[(bits >> 6) & 63] : '=';
                        r += i+2 < s.length ? _chars[bits & 63] : '=';
                    }
                    return r;
                };
                globalThis.atob = function(s) {
                    s = s.replace(/=+$/, '');
                    let r = '';
                    for (let i = 0; i < s.length; i += 4) {
                        const a = _chars.indexOf(s[i]), b = _chars.indexOf(s[i+1]);
                        const c = _chars.indexOf(s[i+2]), d = _chars.indexOf(s[i+3]);
                        const bits = (a << 18) | (b << 12) | ((c >= 0 ? c : 0) << 6) | (d >= 0 ? d : 0);
                        r += String.fromCharCode((bits >> 16) & 255);
                        if (c >= 0) r += String.fromCharCode((bits >> 8) & 255);
                        if (d >= 0) r += String.fromCharCode(bits & 255);
                    }
                    return r;
                };
            }

            // ── localStorage stub (in-memory) ──
            if (typeof globalThis.localStorage === 'undefined') {
                const _ls = {};
                globalThis.localStorage = {
                    getItem: function(k) { return _ls[k] !== undefined ? _ls[k] : null; },
                    setItem: function(k, v) { _ls[k] = String(v); },
                    removeItem: function(k) { delete _ls[k]; },
                    clear: function() { for (const k in _ls) delete _ls[k]; },
                    get length() { return Object.keys(_ls).length; },
                    key: function(i) { return Object.keys(_ls)[i] || null; },
                };
            }

            // ── IndexedDB stub (in-memory) ──
            if (typeof globalThis.indexedDB === 'undefined') {
                const stores = {};

                function IDBRequest() {
                    this.result = null;
                    this.error = null;
                    this.onsuccess = null;
                    this.onerror = null;
                    this.onupgradeneeded = null;
                    this._listeners = {};
                    this.readyState = 'pending';
                    this.transaction = null;
                }
                IDBRequest.prototype._succeed = function(val) {
                    this.result = val;
                    this.readyState = 'done';
                    const evt = { target: this, type: 'success' };
                    if (this.onsuccess) this.onsuccess(evt);
                    const ls = this._listeners['success'] || [];
                    for (const l of ls) l(evt);
                };
                IDBRequest.prototype._fail = function(err) {
                    this.error = err;
                    this.readyState = 'done';
                    const evt = { target: this, type: 'error' };
                    if (this.onerror) this.onerror(evt);
                    const ls = this._listeners['error'] || [];
                    for (const l of ls) l(evt);
                };
                IDBRequest.prototype.addEventListener = function(type, cb) {
                    if (!this._listeners[type]) this._listeners[type] = [];
                    this._listeners[type].push(cb);
                };
                IDBRequest.prototype.removeEventListener = function(type, cb) {
                    if (!this._listeners[type]) return;
                    this._listeners[type] = this._listeners[type].filter(function(l) { return l !== cb; });
                };

                // DOMStringList-like object
                function makeDOMStringList(arr) {
                    const o = {
                        _items: arr,
                        get length() { return this._items.length; },
                        item: function(i) { return this._items[i] || null; },
                        contains: function(s) { return this._items.indexOf(s) !== -1; },
                    };
                    // Make it array-like / iterable
                    for (let i = 0; i < arr.length; i++) o[i] = arr[i];
                    if (typeof Symbol !== 'undefined' && Symbol.iterator) {
                        o[Symbol.iterator] = function() { return this._items[Symbol.iterator](); };
                    }
                    return o;
                }

                function IDBKeyRange() {}
                IDBKeyRange.only = function(val) { return { lower: val, upper: val, lowerOpen: false, upperOpen: false, includes: function(v) { return v === val; } }; };
                IDBKeyRange.lowerBound = function(val, open) { return { lower: val, upper: undefined, lowerOpen: !!open, upperOpen: true }; };
                IDBKeyRange.upperBound = function(val, open) { return { lower: undefined, upper: val, lowerOpen: true, upperOpen: !!open }; };
                IDBKeyRange.bound = function(l, u, lo, uo) { return { lower: l, upper: u, lowerOpen: !!lo, upperOpen: !!uo }; };
                globalThis.IDBKeyRange = IDBKeyRange;

                function IDBCursorWithValue(store, keys, idx) {
                    this._store = store;
                    this._keys = keys;
                    this._idx = idx;
                    if (keys.length > idx) {
                        this.key = keys[idx];
                        this.primaryKey = keys[idx];
                        this.value = stores[store._name] ? stores[store._name][keys[idx]] : undefined;
                    }
                }
                IDBCursorWithValue.prototype.continue = function() {
                    const next = this._idx + 1;
                    const req = this._request;
                    if (next >= this._keys.length) {
                        // No more entries
                        Promise.resolve().then(() => {
                            req.result = null;
                            const evt = { target: req, type: 'success' };
                            if (req.onsuccess) req.onsuccess(evt);
                            const ls = req._listeners['success'] || [];
                            for (const l of ls) l(evt);
                        });
                    } else {
                        const cursor = new IDBCursorWithValue(this._store, this._keys, next);
                        cursor._request = req;
                        Promise.resolve().then(() => {
                            req.result = cursor;
                            const evt = { target: req, type: 'success' };
                            if (req.onsuccess) req.onsuccess(evt);
                            const ls = req._listeners['success'] || [];
                            for (const l of ls) l(evt);
                        });
                    }
                };

                function IDBObjectStore(name, tx) {
                    this._name = name;
                    this.name = name;
                    this.transaction = tx;
                    if (!stores[name]) stores[name] = {};
                    this.indexNames = makeDOMStringList([]);
                    this.keyPath = null;
                    this.autoIncrement = false;
                }
                IDBObjectStore.prototype.put = function(val, key) {
                    const r = new IDBRequest();
                    r.source = this;
                    stores[this._name][key] = val;
                    Promise.resolve().then(function() { r._succeed(key); });
                    return r;
                };
                IDBObjectStore.prototype.add = function(val, key) {
                    return this.put(val, key);
                };
                IDBObjectStore.prototype.get = function(key) {
                    const r = new IDBRequest();
                    r.source = this;
                    const s = this._name;
                    Promise.resolve().then(function() { r._succeed(stores[s][key]); });
                    return r;
                };
                IDBObjectStore.prototype.delete = function(key) {
                    const r = new IDBRequest();
                    r.source = this;
                    delete stores[this._name][key];
                    Promise.resolve().then(function() { r._succeed(undefined); });
                    return r;
                };
                IDBObjectStore.prototype.getAll = function() {
                    const r = new IDBRequest();
                    r.source = this;
                    const s = this._name;
                    Promise.resolve().then(function() { r._succeed(Object.values(stores[s])); });
                    return r;
                };
                IDBObjectStore.prototype.getAllKeys = function() {
                    const r = new IDBRequest();
                    r.source = this;
                    const s = this._name;
                    Promise.resolve().then(function() { r._succeed(Object.keys(stores[s])); });
                    return r;
                };
                IDBObjectStore.prototype.clear = function() {
                    const r = new IDBRequest();
                    r.source = this;
                    stores[this._name] = {};
                    Promise.resolve().then(function() { r._succeed(undefined); });
                    return r;
                };
                IDBObjectStore.prototype.count = function() {
                    const r = new IDBRequest();
                    r.source = this;
                    const s = this._name;
                    Promise.resolve().then(function() { r._succeed(Object.keys(stores[s]).length); });
                    return r;
                };
                IDBObjectStore.prototype.openCursor = function(range, direction) {
                    const r = new IDBRequest();
                    r.source = this;
                    const s = this._name;
                    const st = this;
                    Promise.resolve().then(function() {
                        const keys = Object.keys(stores[s] || {});
                        if (keys.length === 0) {
                            r._succeed(null);
                        } else {
                            const cursor = new IDBCursorWithValue(st, keys, 0);
                            cursor._request = r;
                            r._succeed(cursor);
                        }
                    });
                    return r;
                };
                IDBObjectStore.prototype.openKeyCursor = function(range, direction) {
                    return this.openCursor(range, direction);
                };
                IDBObjectStore.prototype.createIndex = function(name, keyPath, opts) {
                    return { name: name, keyPath: keyPath, unique: !!(opts && opts.unique) };
                };
                IDBObjectStore.prototype.index = function(name) {
                    // Return a stub index that delegates to the store
                    const store = this;
                    return {
                        name: name,
                        openCursor: function(range, dir) { return store.openCursor(range, dir); },
                        openKeyCursor: function(range, dir) { return store.openKeyCursor(range, dir); },
                        get: function(key) { return store.get(key); },
                        getAll: function() { return store.getAll(); },
                        getAllKeys: function() { return store.getAllKeys(); },
                        count: function() { return store.count(); },
                    };
                };

                function IDBTransaction(db, names, mode) {
                    this._db = db;
                    this._names = names;
                    this.mode = mode || 'readonly';
                    this.oncomplete = null;
                    this.onerror = null;
                    this.onabort = null;
                    this._listeners = {};
                    this.error = null;
                    this.db = db;
                    // Fire oncomplete asynchronously
                    const self = this;
                    Promise.resolve().then(function() {
                        Promise.resolve().then(function() {
                            const evt = { target: self, type: 'complete' };
                            if (self.oncomplete) self.oncomplete(evt);
                            const ls = self._listeners['complete'] || [];
                            for (const l of ls) l(evt);
                        });
                    });
                }
                IDBTransaction.prototype.objectStore = function(name) { return new IDBObjectStore(name, this); };
                IDBTransaction.prototype.addEventListener = function(type, cb) {
                    if (!this._listeners[type]) this._listeners[type] = [];
                    this._listeners[type].push(cb);
                };
                IDBTransaction.prototype.removeEventListener = function(type, cb) {
                    if (!this._listeners[type]) return;
                    this._listeners[type] = this._listeners[type].filter(function(l) { return l !== cb; });
                };
                IDBTransaction.prototype.abort = function() {
                    const evt = { target: this, type: 'abort' };
                    if (this.onabort) this.onabort(evt);
                };

                function IDBDatabase(name) {
                    this.name = name;
                    this._storeNames = [];
                    this.version = 1;
                    this.onversionchange = null;
                    this.onclose = null;
                    this._listeners = {};
                }
                Object.defineProperty(IDBDatabase.prototype, 'objectStoreNames', {
                    get: function() { return makeDOMStringList(this._storeNames); }
                });
                IDBDatabase.prototype.transaction = function(names, mode) {
                    const nameArr = Array.isArray(names) ? names : [names];
                    return new IDBTransaction(this, nameArr, mode);
                };
                IDBDatabase.prototype.createObjectStore = function(name, opts) {
                    if (this._storeNames.indexOf(name) === -1) this._storeNames.push(name);
                    const store = new IDBObjectStore(name, null);
                    if (opts && opts.keyPath) store.keyPath = opts.keyPath;
                    if (opts && opts.autoIncrement) store.autoIncrement = true;
                    return store;
                };
                IDBDatabase.prototype.deleteObjectStore = function(name) {
                    this._storeNames = this._storeNames.filter(function(n) { return n !== name; });
                    delete stores[name];
                };
                IDBDatabase.prototype.close = function() {};
                IDBDatabase.prototype.addEventListener = function(type, cb) {
                    if (!this._listeners[type]) this._listeners[type] = [];
                    this._listeners[type].push(cb);
                };
                IDBDatabase.prototype.removeEventListener = function(type, cb) {
                    if (!this._listeners[type]) return;
                    this._listeners[type] = this._listeners[type].filter(function(l) { return l !== cb; });
                };

                globalThis.indexedDB = {
                    open: function(name, version) {
                        const r = new IDBRequest();
                        const db = new IDBDatabase(name);
                        db.version = version || 1;
                        r.result = db;
                        r.transaction = new IDBTransaction(db, [], 'versionchange');
                        Promise.resolve().then(function() {
                            if (r.onupgradeneeded) r.onupgradeneeded({ target: r, oldVersion: 0, newVersion: version || 1 });
                            r._succeed(db);
                        });
                        return r;
                    },
                    deleteDatabase: function(name) {
                        const r = new IDBRequest();
                        Promise.resolve().then(function() { r._succeed(undefined); });
                        return r;
                    },
                    databases: function() {
                        return Promise.resolve([]);
                    }
                };
            }

            // ── AbortController / AbortSignal ──
            if (typeof globalThis.AbortController === 'undefined') {
                class AbortSignal {
                    constructor() {
                        this.aborted = false;
                        this.reason = undefined;
                        this.onabort = null;
                        this._listeners = [];
                    }
                    addEventListener(type, listener) {
                        if (type === 'abort') this._listeners.push(listener);
                    }
                    removeEventListener(type, listener) {
                        if (type === 'abort') {
                            this._listeners = this._listeners.filter(l => l !== listener);
                        }
                    }
                    throwIfAborted() {
                        if (this.aborted) throw this.reason;
                    }
                    _abort(reason) {
                        if (this.aborted) return;
                        this.aborted = true;
                        this.reason = reason || new DOMException('The operation was aborted.', 'AbortError');
                        if (typeof this.onabort === 'function') this.onabort();
                        for (const l of this._listeners) l();
                    }
                }
                AbortSignal.abort = function(reason) {
                    const s = new AbortSignal();
                    s._abort(reason);
                    return s;
                };
                AbortSignal.timeout = function(ms) {
                    const s = new AbortSignal();
                    setTimeout(() => s._abort(new DOMException('The operation timed out.', 'TimeoutError')), ms);
                    return s;
                };
                globalThis.AbortController = class AbortController {
                    constructor() { this.signal = new AbortSignal(); }
                    abort(reason) { this.signal._abort(reason); }
                };
                globalThis.AbortSignal = AbortSignal;
            }

            // ── Event / EventTarget ──
            if (typeof globalThis.Event === 'undefined') {
                globalThis.Event = class Event {
                    constructor(type, opts) {
                        this.type = type;
                        this.bubbles = !!(opts && opts.bubbles);
                        this.cancelable = !!(opts && opts.cancelable);
                        this.defaultPrevented = false;
                        this.target = null;
                        this.currentTarget = null;
                        this.timeStamp = Date.now();
                    }
                    preventDefault() { this.defaultPrevented = true; }
                    stopPropagation() {}
                    stopImmediatePropagation() {}
                };
            }
            if (typeof globalThis.CustomEvent === 'undefined') {
                globalThis.CustomEvent = class CustomEvent extends Event {
                    constructor(type, opts) {
                        super(type, opts);
                        this.detail = opts && opts.detail !== undefined ? opts.detail : null;
                    }
                };
            }
            if (typeof globalThis.EventTarget === 'undefined') {
                globalThis.EventTarget = class EventTarget {
                    constructor() { this._listeners = {}; }
                    addEventListener(type, cb) {
                        if (!this._listeners[type]) this._listeners[type] = [];
                        this._listeners[type].push(cb);
                    }
                    removeEventListener(type, cb) {
                        if (!this._listeners[type]) return;
                        this._listeners[type] = this._listeners[type].filter(l => l !== cb);
                    }
                    dispatchEvent(event) {
                        event.target = this;
                        event.currentTarget = this;
                        const listeners = this._listeners[event.type] || [];
                        for (const l of listeners) l.call(this, event);
                        const handler = this['on' + event.type];
                        if (typeof handler === 'function') handler.call(this, event);
                        return !event.defaultPrevented;
                    }
                };
            }

            // ── Headers ──
            if (typeof globalThis.Headers === 'undefined') {
                globalThis.Headers = class Headers {
                    constructor(init) {
                        this._map = {};
                        if (init) {
                            if (init instanceof Headers) {
                                init.forEach((v, k) => this.append(k, v));
                            } else if (Array.isArray(init)) {
                                for (const [k, v] of init) this.append(k, v);
                            } else {
                                for (const k of Object.keys(init)) this.append(k, init[k]);
                            }
                        }
                    }
                    append(name, value) {
                        const key = name.toLowerCase();
                        if (this._map[key]) this._map[key] += ', ' + value;
                        else this._map[key] = String(value);
                    }
                    delete(name) { delete this._map[name.toLowerCase()]; }
                    get(name) { return this._map[name.toLowerCase()] || null; }
                    has(name) { return name.toLowerCase() in this._map; }
                    set(name, value) { this._map[name.toLowerCase()] = String(value); }
                    forEach(cb) { for (const k in this._map) cb(this._map[k], k, this); }
                    entries() { return Object.entries(this._map)[Symbol.iterator](); }
                    keys() { return Object.keys(this._map)[Symbol.iterator](); }
                    values() { return Object.values(this._map)[Symbol.iterator](); }
                    [Symbol.iterator]() { return this.entries(); }
                };
            }

            // ── Response (supports base64-encoded binary bodies) ──
            if (typeof globalThis.Response === 'undefined') {
                globalThis.Response = class Response {
                    constructor(body, init) {
                        // body can be: base64 string (from fetch), plain string, or Uint8Array
                        if (body instanceof Uint8Array) {
                            this._bytes = body;
                            this._bodyB64 = null;
                        } else {
                            this._bytes = null;
                            this._bodyB64 = body || '';
                        }
                        this.status = (init && init.status) || 200;
                        this.statusText = (init && init.statusText) || 'OK';
                        this.ok = this.status >= 200 && this.status < 300;
                        this.headers = new Headers((init && init.headers) || {});
                        this.type = 'basic';
                        this.url = '';
                        this._bodyUsed = false;
                        this._isB64 = !!(init && init._isB64);
                    }
                    _getBytes() {
                        if (this._bytes) return this._bytes;
                        if (!this._bodyB64) return new Uint8Array(0);
                        if (this._isB64) {
                            const bin = atob(this._bodyB64);
                            const bytes = new Uint8Array(bin.length);
                            for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
                            return bytes;
                        }
                        return new TextEncoder().encode(this._bodyB64);
                    }
                    get bodyUsed() { return this._bodyUsed; }
                    async text() {
                        this._bodyUsed = true;
                        if (this._bytes) return new TextDecoder().decode(this._bytes);
                        if (this._isB64) return new TextDecoder().decode(this._getBytes());
                        return this._bodyB64 || '';
                    }
                    async json() { return JSON.parse(await this.text()); }
                    async arrayBuffer() {
                        this._bodyUsed = true;
                        return this._getBytes().buffer;
                    }
                    clone() {
                        const r = new Response(this._bodyB64, { status: this.status, statusText: this.statusText, headers: this.headers, _isB64: this._isB64 });
                        if (this._bytes) r._bytes = this._bytes;
                        return r;
                    }
                };
            }

            // ── Request ──
            if (typeof globalThis.Request === 'undefined') {
                globalThis.Request = class Request {
                    constructor(input, init) {
                        this.url = typeof input === 'string' ? input : input.url;
                        this.method = (init && init.method) || 'GET';
                        this.headers = new Headers((init && init.headers) || {});
                        this._body = (init && init.body) || null;
                        this.signal = (init && init.signal) || null;
                    }
                    async text() { return this._body ? String(this._body) : ''; }
                    async json() { return JSON.parse(await this.text()); }
                };
            }

            // ── fetch (wrapping __oubli_fetch host function) ──
            if (typeof globalThis.fetch === 'undefined') {
                globalThis.fetch = async function(input, init) {
                    let url, method, headers, body, signal;
                    if (typeof input === 'string') {
                        url = input;
                    } else if (input instanceof Request) {
                        url = input.url;
                        method = input.method;
                        headers = input.headers;
                        body = input._body;
                        signal = input.signal;
                    } else {
                        url = String(input);
                    }
                    if (init) {
                        if (init.method) method = init.method;
                        if (init.headers) headers = new Headers(init.headers);
                        if (init.body !== undefined) body = init.body;
                        if (init.signal) signal = init.signal;
                    }
                    method = method || 'GET';
                    if (!headers) headers = new Headers();

                    // Serialize headers to JSON
                    const hObj = {};
                    headers.forEach((v, k) => { hObj[k] = v; });
                    const headersJson = JSON.stringify(hObj);

                    // Handle binary request bodies (Uint8Array, ArrayBuffer)
                    let bodyStr = '';
                    if (body) {
                        if (body instanceof Uint8Array) {
                            const chars = [];
                            for (let i = 0; i < body.length; i++) chars.push(String.fromCharCode(body[i]));
                            bodyStr = '__b64:' + btoa(chars.join(''));
                        } else if (body instanceof ArrayBuffer) {
                            const u8 = new Uint8Array(body);
                            const chars = [];
                            for (let i = 0; i < u8.length; i++) chars.push(String.fromCharCode(u8[i]));
                            bodyStr = '__b64:' + btoa(chars.join(''));
                        } else {
                            bodyStr = String(body);
                        }
                    }

                    if (signal && signal.aborted) {
                        throw new DOMException('The operation was aborted.', 'AbortError');
                    }

                    const result = await __oubli_fetch(url, method, headersJson, bodyStr);

                    const status = parseInt(result.status, 10);
                    const respHeaders = JSON.parse(result.headers || '{}');
                    const bodyB64 = result.body_b64 || '';

                    return new Response(bodyB64, {
                        status: status,
                        statusText: status === 200 ? 'OK' : String(status),
                        headers: respHeaders,
                        _isB64: true,
                    });
                };
            }

            // ── DOMException ──
            if (typeof globalThis.DOMException === 'undefined') {
                globalThis.DOMException = class DOMException extends Error {
                    constructor(message, name) {
                        super(message);
                        this.name = name || 'Error';
                    }
                };
            }

            // ── WebSocket stub ──
            // The Atomiq SDK uses WebSocket for LP communication.
            // In QuickJS we route WS through __oubli_fetch or stub it.
            if (typeof globalThis.WebSocket === 'undefined') {
                const WS_CONNECTING = 0, WS_OPEN = 1, WS_CLOSING = 2, WS_CLOSED = 3;
                globalThis.WebSocket = class WebSocket extends EventTarget {
                    static get CONNECTING() { return WS_CONNECTING; }
                    static get OPEN() { return WS_OPEN; }
                    static get CLOSING() { return WS_CLOSING; }
                    static get CLOSED() { return WS_CLOSED; }
                    get CONNECTING() { return WS_CONNECTING; }
                    get OPEN() { return WS_OPEN; }
                    get CLOSING() { return WS_CLOSING; }
                    get CLOSED() { return WS_CLOSED; }

                    constructor(url, protocols) {
                        super();
                        this.url = url;
                        this.protocol = '';
                        this.readyState = WS_CONNECTING;
                        this.bufferedAmount = 0;
                        this.extensions = '';
                        this.binaryType = 'blob';
                        this.onopen = null;
                        this.onclose = null;
                        this.onmessage = null;
                        this.onerror = null;

                        // Auto-open after microtask to mimic browser behavior
                        Promise.resolve().then(() => {
                            this.readyState = WS_OPEN;
                            this.dispatchEvent(new Event('open'));
                        });
                    }
                    send(data) {
                        if (this.readyState !== WS_OPEN) throw new DOMException('WebSocket is not open');
                    }
                    close(code, reason) {
                        this.readyState = WS_CLOSED;
                        this.dispatchEvent(new Event('close'));
                    }
                };
            }

            // ── Blob stub ──
            if (typeof globalThis.Blob === 'undefined') {
                globalThis.Blob = class Blob {
                    constructor(parts, opts) {
                        this._parts = parts || [];
                        this.type = (opts && opts.type) || '';
                        this.size = this._parts.reduce((s, p) => s + (typeof p === 'string' ? p.length : p.byteLength || 0), 0);
                    }
                    async text() { return this._parts.map(p => typeof p === 'string' ? p : new TextDecoder().decode(p)).join(''); }
                    async arrayBuffer() { const t = await this.text(); return new TextEncoder().encode(t).buffer; }
                    slice(start, end, type) { return new Blob([this._parts.join('').slice(start, end)], { type }); }
                };
            }

            // ── File stub ──
            if (typeof globalThis.File === 'undefined') {
                globalThis.File = class File extends Blob {
                    constructor(parts, name, opts) {
                        super(parts, opts);
                        this.name = name;
                        this.lastModified = (opts && opts.lastModified) || Date.now();
                    }
                };
            }

            // ── Buffer polyfill (Node.js compatible subset) ──
            if (typeof globalThis.Buffer === 'undefined') {
                const _b64chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
                const _b64lookup = new Uint8Array(256);
                for (let i = 0; i < _b64chars.length; i++) _b64lookup[_b64chars.charCodeAt(i)] = i;

                class Buffer extends Uint8Array {
                    static from(data, encoding) {
                        if (typeof data === 'string') {
                            if (encoding === 'base64') {
                                // Decode base64
                                const str = data.replace(/[^A-Za-z0-9+/]/g, '');
                                const len = str.length;
                                const bytes = new Uint8Array(Math.floor(len * 3 / 4));
                                let p = 0;
                                for (let i = 0; i < len; i += 4) {
                                    const a = _b64lookup[str.charCodeAt(i)];
                                    const b = _b64lookup[str.charCodeAt(i+1)];
                                    const c = _b64lookup[str.charCodeAt(i+2)];
                                    const d = _b64lookup[str.charCodeAt(i+3)];
                                    bytes[p++] = (a << 2) | (b >> 4);
                                    if (i+2 < len) bytes[p++] = ((b & 15) << 4) | (c >> 2);
                                    if (i+3 < len) bytes[p++] = ((c & 3) << 6) | d;
                                }
                                return Buffer._wrap(bytes.subarray(0, p));
                            } else if (encoding === 'hex') {
                                const bytes = new Uint8Array(data.length / 2);
                                for (let i = 0; i < data.length; i += 2) {
                                    bytes[i/2] = parseInt(data.substring(i, i+2), 16);
                                }
                                return Buffer._wrap(bytes);
                            } else {
                                // UTF-8
                                const enc = new TextEncoder();
                                return Buffer._wrap(enc.encode(data));
                            }
                        } else if (data instanceof ArrayBuffer) {
                            return Buffer._wrap(new Uint8Array(data));
                        } else if (ArrayBuffer.isView(data)) {
                            return Buffer._wrap(new Uint8Array(data.buffer, data.byteOffset, data.byteLength));
                        } else if (Array.isArray(data)) {
                            return Buffer._wrap(new Uint8Array(data));
                        }
                        return Buffer._wrap(new Uint8Array(0));
                    }

                    static _wrap(arr) {
                        Object.setPrototypeOf(arr, Buffer.prototype);
                        return arr;
                    }

                    static alloc(size, fill) {
                        const buf = new Uint8Array(size);
                        if (fill !== undefined) buf.fill(typeof fill === 'number' ? fill : 0);
                        return Buffer._wrap(buf);
                    }

                    static allocUnsafe(size) {
                        return Buffer._wrap(new Uint8Array(size));
                    }

                    static concat(list, totalLength) {
                        if (totalLength === undefined) {
                            totalLength = 0;
                            for (const b of list) totalLength += b.length;
                        }
                        const result = new Uint8Array(totalLength);
                        let offset = 0;
                        for (const b of list) {
                            result.set(b, offset);
                            offset += b.length;
                        }
                        return Buffer._wrap(result);
                    }

                    static isBuffer(obj) {
                        return obj instanceof Buffer || (obj != null && obj._isBuffer === true);
                    }

                    static isView(obj) {
                        return ArrayBuffer.isView(obj);
                    }

                    static isEncoding(enc) {
                        return ['utf8','utf-8','hex','base64','ascii','latin1','binary'].indexOf(enc) !== -1;
                    }

                    get _isBuffer() { return true; }

                    toString(encoding) {
                        if (encoding === 'hex') {
                            let hex = '';
                            for (let i = 0; i < this.length; i++) {
                                hex += (this[i] < 16 ? '0' : '') + this[i].toString(16);
                            }
                            return hex;
                        } else if (encoding === 'base64') {
                            let result = '';
                            for (let i = 0; i < this.length; i += 3) {
                                const a = this[i], b = this[i+1], c = this[i+2];
                                const bits = (a << 16) | ((b || 0) << 8) | (c || 0);
                                result += _b64chars[(bits >> 18) & 63] + _b64chars[(bits >> 12) & 63];
                                result += i+1 < this.length ? _b64chars[(bits >> 6) & 63] : '=';
                                result += i+2 < this.length ? _b64chars[bits & 63] : '=';
                            }
                            return result;
                        } else {
                            const dec = new TextDecoder();
                            return dec.decode(this);
                        }
                    }

                    slice(start, end) {
                        const sliced = Uint8Array.prototype.slice.call(this, start, end);
                        return Buffer._wrap(sliced);
                    }

                    subarray(start, end) {
                        const sub = Uint8Array.prototype.subarray.call(this, start, end);
                        return Buffer._wrap(sub);
                    }

                    copy(target, targetStart, sourceStart, sourceEnd) {
                        targetStart = targetStart || 0;
                        sourceStart = sourceStart || 0;
                        sourceEnd = sourceEnd || this.length;
                        for (let i = sourceStart; i < sourceEnd; i++) {
                            target[targetStart + i - sourceStart] = this[i];
                        }
                        return sourceEnd - sourceStart;
                    }

                    equals(other) {
                        if (this.length !== other.length) return false;
                        for (let i = 0; i < this.length; i++) {
                            if (this[i] !== other[i]) return false;
                        }
                        return true;
                    }

                    compare(other) {
                        const len = Math.min(this.length, other.length);
                        for (let i = 0; i < len; i++) {
                            if (this[i] < other[i]) return -1;
                            if (this[i] > other[i]) return 1;
                        }
                        if (this.length < other.length) return -1;
                        if (this.length > other.length) return 1;
                        return 0;
                    }

                    write(string, offset, length, encoding) {
                        offset = offset || 0;
                        const bytes = Buffer.from(string, encoding || 'utf8');
                        const len = Math.min(bytes.length, (length || this.length) - offset);
                        for (let i = 0; i < len; i++) this[offset + i] = bytes[i];
                        return len;
                    }

                    readUInt8(offset) { return this[offset]; }
                    readUInt16BE(offset) { return (this[offset] << 8) | this[offset+1]; }
                    readUInt32BE(offset) { return (this[offset] * 0x1000000) + ((this[offset+1] << 16) | (this[offset+2] << 8) | this[offset+3]); }
                    readUInt16LE(offset) { return this[offset] | (this[offset+1] << 8); }
                    readUInt32LE(offset) { return this[offset] | (this[offset+1] << 8) | (this[offset+2] << 16) | (this[offset+3] * 0x1000000); }
                    writeUInt8(value, offset) { this[offset] = value & 0xff; return offset + 1; }
                    writeUInt16BE(value, offset) { this[offset] = (value >> 8) & 0xff; this[offset+1] = value & 0xff; return offset + 2; }
                    writeUInt32BE(value, offset) { this[offset] = (value >> 24) & 0xff; this[offset+1] = (value >> 16) & 0xff; this[offset+2] = (value >> 8) & 0xff; this[offset+3] = value & 0xff; return offset + 4; }

                    toJSON() { return { type: 'Buffer', data: Array.from(this) }; }
                }

                globalThis.Buffer = Buffer;
            }

            // ── Promise.any polyfill (ES2021, not in QuickJS ES2020) ──
            if (typeof Promise.any === 'undefined') {
                Promise.any = function(promises) {
                    return new Promise(function(resolve, reject) {
                        var errors = [];
                        var remaining = 0;
                        var arr = Array.from(promises);
                        if (arr.length === 0) {
                            reject(new AggregateError([], 'All promises were rejected'));
                            return;
                        }
                        remaining = arr.length;
                        arr.forEach(function(p, i) {
                            Promise.resolve(p).then(resolve, function(err) {
                                errors[i] = err;
                                remaining--;
                                if (remaining === 0) {
                                    reject(new AggregateError(errors, 'All promises were rejected'));
                                }
                            });
                        });
                    });
                };
            }
            if (typeof globalThis.AggregateError === 'undefined') {
                globalThis.AggregateError = class AggregateError extends Error {
                    constructor(errors, message) {
                        super(message);
                        this.errors = errors;
                        this.name = 'AggregateError';
                    }
                };
            }

            // ── console.log → __oubli_log ──
            if (typeof globalThis.console === 'undefined' || typeof globalThis.console.log !== 'function') {
                globalThis.console = {
                    log: function() { __oubli_log('info', Array.from(arguments).join(' ')); },
                    info: function() { __oubli_log('info', Array.from(arguments).join(' ')); },
                    warn: function() { __oubli_log('warn', Array.from(arguments).join(' ')); },
                    error: function() { __oubli_log('error', Array.from(arguments).join(' ')); },
                    debug: function() { __oubli_log('debug', Array.from(arguments).join(' ')); },
                };
            }
        "#;

        async_with!(self.ctx => |ctx| {
            ctx.eval::<(), _>(polyfill)
                .catch(&ctx)
                .map_err(|e| SwapError::Runtime(format!("Failed to install polyfills: {:?}", e)))
        })
        .await
    }

    /// Load the bundled Atomiq SDK JS.
    async fn load_bundle(&self) -> std::result::Result<(), SwapError> {
        let bundle = include_str!("../js/bundle.js");

        async_with!(self.ctx => |ctx| {
            ctx.eval::<(), _>(bundle)
                .catch(&ctx)
                .map_err(|e| SwapError::Runtime(format!("Failed to load JS bundle: {:?}", e)))
        })
        .await
    }

    /// Call a JS async function on the __oubli_swap global and return the JSON result.
    pub async fn call_js_fn(
        &self,
        fn_name: &str,
        args_json: &[&str],
    ) -> std::result::Result<String, SwapError> {
        let fn_name = fn_name.to_string();
        let args: Vec<String> = args_json.iter().map(|s| s.to_string()).collect();

        async_with!(self.ctx => |ctx| {
            let globals = ctx.globals();
            let swap_obj: Object = globals
                .get("__oubli_swap")
                .map_err(|e| SwapError::Runtime(format!("__oubli_swap not found: {:?}", e)))?;

            let func: Function = swap_obj
                .get(&*fn_name)
                .map_err(|e| SwapError::Runtime(format!("Function {} not found: {:?}", fn_name, e)))?;

            // Call with the right arity
            let result_val: Value = match args.len() {
                0 => func.call(()),
                1 => func.call((args[0].clone(),)),
                2 => func.call((args[0].clone(), args[1].clone())),
                3 => func.call((args[0].clone(), args[1].clone(), args[2].clone())),
                _ => return Err(SwapError::Runtime("Too many arguments".into())),
            }
            .catch(&ctx)
            .map_err(|e| SwapError::Execution(format!("{:?}", e)))?;

            // If it's a promise, await it
            if result_val.is_promise() {
                let promise = result_val.into_promise().unwrap();
                let resolved: String = promise
                    .into_future()
                    .await
                    .catch(&ctx)
                    .map_err(|e| SwapError::Execution(format!("{:?}", e)))?;
                Ok(resolved)
            } else {
                // Synchronous result
                let result: String = result_val
                    .get()
                    .map_err(|e| SwapError::Execution(format!("Not a string: {:?}", e)))?;
                Ok(result)
            }
        })
        .await
    }
}
