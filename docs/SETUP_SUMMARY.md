# Setup Summary - Mnemonic to Private Key

## ✅ What We've Implemented

You asked about converting your mnemonic to a private key for Celestia DA posting. Here's what we've done:

### 1. **Automatic Mnemonic Conversion** 🔑

Added a `crypto` module that handles:
- BIP39 mnemonic parsing and validation
- Seed generation from mnemonic
- SLIP-10 ed25519 key derivation with Cosmos path `m/44'/118'/0'/0/0`
- Hex encoding for use with `celestia-client`

### 2. **Flexible Configuration** ⚙️

You can now use **either**:

```toml
# Option A: Mnemonic (easier to remember/backup)
[celestia]
mnemonic = "your twenty four word mnemonic phrase here"
```

**OR**

```toml
# Option B: Direct private key hex
[celestia]
private_key_hex = "393fdb5def075819de55756b45c9e2c8531a8c78dd6eede483d3440e9457d839"
```

### 3. **Validation Built-In** ✨

The config loader now:
- Ensures exactly ONE auth method is provided
- Validates mnemonic format (BIP39)
- Validates private key length (32 bytes = 64 hex chars)
- Gives clear error messages if something's wrong

## 📝 Your Action Items

### Step 1: Add Your Mnemonic to Config

Edit `config.toml`:

```toml
[celestia]
rpc_url = "ws://localhost:26658"      # Your Celestia node RPC
grpc_url = "http://localhost:9090"    # Your Celestia node gRPC
namespace = "0x2N1CE"                 # Your namespace
poster_mode = "mock"                   # Change to "real" when ready

# Add your mnemonic here (uncomment and fill in):
mnemonic = "word1 word2 word3 ... word24"

# Don't use both! Comment out private_key_hex if using mnemonic
# private_key_hex = "..."
```

### Step 2: Test the Configuration

```bash
cargo build
cargo run

# You should see:
# ✅ Loaded config: ...
# 🚀 Listening for OTLP/HTTP on http://0.0.0.0:4318
```

If there's an error with your mnemonic, you'll get a clear message.

### Step 3: Secure Your Config (Important!)

**For development:**
```bash
# Make sure config.toml is in .gitignore (it should be)
git status  # config.toml should NOT appear here
```

**For production:**
Consider using environment variables instead:
```bash
export CELESTIA_MNEMONIC="your mnemonic here"
```

## 🔐 How It Works

The conversion happens automatically when the app starts:

```
Your Mnemonic
    ↓
BIP39 Parser (validates format)
    ↓
Seed Generation (512 bits)
    ↓
SLIP-10 Derivation (m/44'/118'/0'/0/0)
    ↓
Private Key (32 bytes)
    ↓
Hex Encode (64 characters)
    ↓
Ready for celestia-client!
```

This happens **once** at startup, then the hex key is used throughout.

## 📚 More Information

See detailed documentation:
- **`docs/MNEMONIC_TO_KEY.md`** - Technical details on the conversion
- **`docs/DA_BLOB.md`** - Celestia blob posting example

## 🛠️ Dependencies Added

We added these crates to handle the conversion:

```toml
bip39 = "2.1"              # Mnemonic parsing & seed generation
slip10_ed25519 = "0.1"     # Ed25519 key derivation for Cosmos
celestia-client = "0.2"    # Celestia DA client
```

## Next Steps

Once your mnemonic is configured:
1. ✅ The crypto conversion is ready
2. 🚧 Next: Implement DA posting in `src/da/mod.rs`
3. 🚧 Then: Add ZK proof generation

Want to proceed with implementing the actual DA blob posting?

