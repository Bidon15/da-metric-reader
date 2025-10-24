# Environment Variable Setup

## Overview

The `da-reader` uses environment variables for secure credential management. Your mnemonic or private key should **never** be committed to git!

## Quick Setup

### 1. Copy the Template

```bash
cp .env.example .env
```

### 2. Edit `.env` with Your Credentials

```bash
# Open in your editor
nano .env
# or
vim .env
# or
code .env
```

Add your mnemonic:

```bash
# Your 24-word mnemonic phrase
CELESTIA_MNEMONIC=word1 word2 word3 ... word24

# OR use private key directly:
# CELESTIA_PRIVATE_KEY=393fdb5def075819de55756b45c9e2c8531a8c78dd6eede483d3440e9457d839
```

### 3. Verify `.env` is Ignored by Git

```bash
git status

# .env should NOT appear in the list!
# If it does, make sure .gitignore contains:
# .env
```

## How It Works

The application loads credentials in this order:

```
1. Load config.toml (contains defaults, no secrets)
   ↓
2. Load .env file (if exists)
   ↓
3. Check environment variables:
   - CELESTIA_MNEMONIC
   - CELESTIA_PRIVATE_KEY
   ↓
4. Environment variables OVERRIDE config.toml values
   ↓
5. Validate exactly ONE auth method is provided
   ↓
6. Convert mnemonic → private key (if needed)
   ↓
7. Ready to use!
```

## Environment Variables

### CELESTIA_MNEMONIC

Your 24-word BIP39 mnemonic phrase. Automatically converted to private key using Cosmos derivation path `m/44'/118'/0'/0/0`.

**Example:**

```bash
CELESTIA_MNEMONIC=abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about
```

### CELESTIA_PRIVATE_KEY

Direct ed25519 private key in hex format (64 characters = 32 bytes).

**Example:**

```bash
CELESTIA_PRIVATE_KEY=393fdb5def075819de55756b45c9e2c8531a8c78dd6eede483d3440e9457d839
```

**Note:** Provide **ONLY ONE** (mnemonic OR private_key), not both!

## Configuration Priority

Environment variables **always override** config.toml settings:

```toml
# config.toml
[celestia]
mnemonic = "test mnemonic"  # ← This will be IGNORED if CELESTIA_MNEMONIC is set
```

```bash
# .env
CELESTIA_MNEMONIC=your real mnemonic here  # ← This takes precedence!
```

## Different Environments

### Development

Use `.env` file:

```bash
# .env
CELESTIA_MNEMONIC=your dev mnemonic
```

### Production

Set environment variables directly (don't use `.env` file in production):

```bash
# Docker
docker run -e CELESTIA_MNEMONIC="your prod mnemonic" da-reader

# Kubernetes
kubectl create secret generic celestia-auth \
  --from-literal=CELESTIA_MNEMONIC='your prod mnemonic'

# systemd service
Environment="CELESTIA_MNEMONIC=your prod mnemonic"
```

### CI/CD

Use secrets management:

**GitHub Actions:**

```yaml
env:
  CELESTIA_MNEMONIC: ${{ secrets.CELESTIA_MNEMONIC }}
```

**GitLab CI:**

```yaml
variables:
  CELESTIA_MNEMONIC: $CELESTIA_MNEMONIC # Set in CI/CD variables
```

## Troubleshooting

### Error: "Must provide either 'mnemonic' or 'private_key_hex'"

**Problem:** No authentication method found.

**Solution:**

1. Check `.env` file exists: `ls -la .env`
2. Verify it contains `CELESTIA_MNEMONIC=...`
3. Make sure there are no typos in the variable name

### Error: "Provide only ONE of 'mnemonic' or 'private_key_hex'"

**Problem:** Both methods are set.

**Solution:**

```bash
# In .env, comment out one:
CELESTIA_MNEMONIC=your mnemonic here
# CELESTIA_PRIVATE_KEY=...  # ← Commented out
```

### Error: "Failed to parse mnemonic"

**Problem:** Invalid mnemonic format.

**Solution:**

- Ensure it's 12 or 24 words
- Check for typos in words
- Verify words are from BIP39 wordlist
- Make sure words are space-separated

### Mnemonic Not Loading

**Debug steps:**

1. **Check file exists:**

   ```bash
   cat .env
   ```

2. **Check for syntax errors:**

   ```bash
   # Should have no quotes around value
   CELESTIA_MNEMONIC=word1 word2 word3...  # ✅ Correct
   CELESTIA_MNEMONIC="word1 word2..."      # ❌ Wrong (no quotes needed)
   ```

3. **Test manually:**
   ```bash
   export CELESTIA_MNEMONIC="your mnemonic"
   cargo run
   ```

## Security Best Practices

### ✅ DO

- ✅ Use `.env` file for local development
- ✅ Set `.env` in `.gitignore`
- ✅ Use different mnemonics for dev/prod
- ✅ Use secrets management in production
- ✅ Rotate keys regularly

### ❌ DON'T

- ❌ Commit `.env` to git
- ❌ Store mnemonics in `config.toml`
- ❌ Share mnemonics in chat/email
- ❌ Use production keys in development
- ❌ Hardcode keys in source code

## File Checklist

```bash
✅ .env.example    # Committed to git (template)
✅ .gitignore      # Contains .env
❌ .env            # NOT in git (your secrets)
✅ config.toml     # Committed (no secrets)
```

Verify:

```bash
git ls-files | grep -E '\.(env|toml)$'
# Should show:
# .env.example  ✅
# config.toml   ✅
#
# Should NOT show:
# .env          ❌ (if this appears, you have a problem!)
```

## Example Setup Session

```bash
# 1. Clone repo
git clone <repo>
cd da-reader

# 2. Copy env template
cp .env.example .env

# 3. Edit with your mnemonic
echo 'CELESTIA_MNEMONIC=word1 word2 ... word24' > .env

# 4. Verify it's ignored
git status  # .env should NOT appear

# 5. Run
cargo run

# 6. Should see:
# ✅ Loaded config: ...
# 🔑 Using mnemonic authentication
# 🚀 Listening on http://0.0.0.0:4318
```

## Additional Resources

- See `MNEMONIC_TO_KEY.md` for technical details on key derivation
- See `SETUP_SUMMARY.md` for overall setup guide
