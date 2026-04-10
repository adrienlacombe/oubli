use std::env;
use std::fs;
use std::path::Path;

const XOR_KEY: u8 = 0xAB;

fn obfuscate(input: &str) -> String {
    let bytes: Vec<String> = input
        .bytes()
        .map(|b| format!("0x{:02x}", b ^ XOR_KEY))
        .collect();
    format!("&[{}]", bytes.join(", "))
}

fn main() {
    println!("cargo:rerun-if-env-changed=OUBLI_MAINNET_RPC_URL");
    println!("cargo:rerun-if-env-changed=OUBLI_MAINNET_PAYMASTER_API_KEY");
    println!("cargo:rerun-if-env-changed=OUBLI_FEE_COLLECTOR_PUBKEY");
    println!("cargo:rerun-if-env-changed=OUBLI_FEE_PERCENT");
    println!("cargo:rerun-if-env-changed=OUBLI_SEPOLIA_RPC_URL");
    println!("cargo:rerun-if-env-changed=OUBLI_SEPOLIA_PAYMASTER_API_KEY");

    let mainnet_rpc = env::var("OUBLI_MAINNET_RPC_URL").unwrap_or_default();
    let mainnet_paymaster = env::var("OUBLI_MAINNET_PAYMASTER_API_KEY").unwrap_or_default();
    let mainnet_fee_collector = env::var("OUBLI_FEE_COLLECTOR_PUBKEY").unwrap_or_default();
    let mainnet_fee_percent = env::var("OUBLI_FEE_PERCENT").unwrap_or_default();
    let sepolia_rpc = env::var("OUBLI_SEPOLIA_RPC_URL").unwrap_or_default();
    let sepolia_paymaster = env::var("OUBLI_SEPOLIA_PAYMASTER_API_KEY").unwrap_or_default();

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("secrets.rs");

    let contents = format!(
        r#"const XOR_KEY: u8 = 0x{XOR_KEY:02x};

const MAINNET_RPC_ENC: &[u8] = {mainnet_rpc};
const MAINNET_PAYMASTER_ENC: &[u8] = {mainnet_paymaster};
const MAINNET_FEE_COLLECTOR_ENC: &[u8] = {mainnet_fee_collector};
const MAINNET_FEE_PERCENT_ENC: &[u8] = {mainnet_fee_percent};
const SEPOLIA_RPC_ENC: &[u8] = {sepolia_rpc};
const SEPOLIA_PAYMASTER_ENC: &[u8] = {sepolia_paymaster};

fn decode(enc: &[u8]) -> String {{
    enc.iter().map(|b| (b ^ XOR_KEY) as char).collect()
}}
"#,
        mainnet_rpc = obfuscate(&mainnet_rpc),
        mainnet_paymaster = obfuscate(&mainnet_paymaster),
        mainnet_fee_collector = obfuscate(&mainnet_fee_collector),
        mainnet_fee_percent = obfuscate(&mainnet_fee_percent),
        sepolia_rpc = obfuscate(&sepolia_rpc),
        sepolia_paymaster = obfuscate(&sepolia_paymaster),
    );

    fs::write(dest, contents).unwrap();
}
