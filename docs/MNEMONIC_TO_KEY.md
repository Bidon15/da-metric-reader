# Mnemonic to Private Key Conversion

## Overview

The `da-reader` supports two ways to authenticate with Celestia DA:

1. **Mnemonic Phrase** (24 words) - User-friendly, automatically converted
2. **Private Key Hex** (64 characters) - Direct key, no conversion needed

## Conversion Process

When you provide a mnemonic, it's automatically converted to a private key using this flow:

```
┌─────────────────────────────────────────────────────────────┐
│  Mnemonic (24 words)                                        │
│  "word1 word2 word3 ... word24"                             │
└──────────────────┬──────────────────────────────────────────┘
                   │
                   │ BIP39: Mnemonic → Seed
                   ▼
┌─────────────────────────────────────────────────────────────┐
│  Seed (512 bits)                                            │
│  Binary seed derived from mnemonic + optional passphrase    │
└──────────────────┬──────────────────────────────────────────┘
                   │
                   │ SLIP-10 Derivation
                   │ Path: m/44'/118'/0'/0/0
                   ▼
┌─────────────────────────────────────────────────────────────┐
│  Private Key (32 bytes)                                     │
│  ed25519 private key for signing                            │
└──────────────────┬──────────────────────────────────────────┘
                   │
                   │ Hex Encode
                   ▼
┌─────────────────────────────────────────────────────────────┐
│  Private Key Hex (64 characters)                            │
│  "393fdb5def075819de55756b45c9e2c8531a8c78dd6eed..."       │
└─────────────────────────────────────────────────────────────┘
```

## Derivation Path Explained

The path `m/44'/118'/0'/0/0` breaks down as:

| Level | Value | Meaning                         | Hardened? |
| ----- | ----- | ------------------------------- | --------- |
| 0     | 44'   | BIP44 purpose                   | Yes       |
| 1     | 118'  | Cosmos coin type (ATOM)         | Yes       |
| 2     | 0'    | Account index                   | Yes       |
| 3     | 0     | Change (0=external, 1=internal) | No        |
| 4     | 0     | Address index                   | No        |

**Note:** The `'` (apostrophe) indicates **hardened derivation**, which uses `0x80000000 | index` for enhanced security.

## Configuration

In your `config.toml`, provide **ONE** of these options:

### Option 1: Mnemonic (Recommended for Development)

```toml
[celestia]
rpc_url = "ws://localhost:26658"
grpc_url = "http://localhost:9090"
namespace = "0x2N1CE"
mnemonic = "your twenty four word mnemonic phrase goes here with spaces between each word"
```

### Option 2: Private Key Hex

```toml
[celestia]
rpc_url = "ws://localhost:26658"
grpc_url = "http://localhost:9090"
namespace = "0x2N1CE"
private_key_hex = "393fdb5def075819de55756b45c9e2c8531a8c78dd6eede483d3440e9457d839"
```

## Security Notes

### ⚠️ Important Security Practices

1. **Never commit mnemonics or keys to git**

   - Use environment variables or secure vaults in production
   - The `config.toml` should be in `.gitignore` if it contains secrets

2. **Use different keys for different environments**

   - Development: Test mnemonics are okay
   - Production: Use hardware wallets or secure key management

3. **Mnemonic vs Private Key**
   - Mnemonic: Easier to backup and restore
   - Private Key: Direct control, no derivation needed

### For Production

Consider using environment variables:

```toml
[celestia]
rpc_url = "ws://localhost:26658"
grpc_url = "http://localhost:9090"
namespace = "0x2N1CE"
# Set via environment: export CELESTIA_MNEMONIC="your mnemonic here"
mnemonic = "${CELESTIA_MNEMONIC}"
```

Or use a secrets management service (HashiCorp Vault, AWS Secrets Manager, etc.)

## How It Works in Code

The conversion is handled automatically when loading config:

```rust
// In config.rs
impl CelestiaConfig {
    pub fn get_private_key_hex(&self) -> anyhow::Result<String> {
        if let Some(hex) = &self.private_key_hex {
            // Direct hex key provided - validate and use
            crate::crypto::validate_private_key_hex(hex)?;
            Ok(hex.clone())
        } else if let Some(mnemonic) = &self.mnemonic {
            // Mnemonic provided - derive the key
            crate::crypto::mnemonic_to_private_key_hex(mnemonic)
        } else {
            anyhow::bail!("No authentication method provided")
        }
    }
}
```

The conversion happens **once** at startup, so there's no performance overhead.

## Validation

The config loader validates that:

1. **Exactly one** auth method is provided (not both, not neither)
2. If hex key: Must be exactly 64 hex characters (32 bytes)
3. If mnemonic: Must be valid BIP39 (12 or 24 words)
4. Derivation succeeds without errors

If validation fails, you'll get a clear error message on startup.

## Testing Your Mnemonic

You can test mnemonic conversion without running the full service:

```rust
use da_reader::crypto::mnemonic_to_private_key_hex;

fn main() {
    let mnemonic = "your test mnemonic here";

    match mnemonic_to_private_key_hex(mnemonic) {
        Ok(hex) => println!("Private key: {}", hex),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## References

- [BIP39 Specification](https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki) - Mnemonic generation
- [BIP44 Specification](https://github.com/bitcoin/bips/blob/master/bip-0044.mediawiki) - HD wallet structure
- [SLIP-10](https://github.com/satoshilabs/slips/blob/master/slip-0010.md) - Ed25519 key derivation
- [Cosmos SDK Key Derivation](https://docs.cosmos.network/v0.46/user/run-node/keyring.html) - Cosmos-specific details
