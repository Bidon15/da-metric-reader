use anyhow::{Context, Result};
use bip39::Mnemonic;
use slip10_ed25519::derive_ed25519_private_key;

/// Derives a private key from a mnemonic phrase
/// 
/// For Celestia/Cosmos chains, this uses:
/// - BIP39 for mnemonic → seed conversion
/// - BIP32/44 derivation path: m/44'/118'/0'/0/0
///   - 44' = BIP44 purpose
///   - 118' = ATOM coin type (Cosmos)
///   - 0' = account
///   - 0 = change
///   - 0 = address index
pub fn mnemonic_to_private_key_hex(mnemonic_str: &str) -> Result<String> {
    // Parse and validate the mnemonic
    let mnemonic = Mnemonic::parse(mnemonic_str)
        .context("Failed to parse mnemonic. Ensure it's a valid BIP39 mnemonic phrase.")?;
    
    // Convert mnemonic to seed (with empty passphrase)
    let seed = mnemonic.to_seed("");
    
    // Derive the private key using the Cosmos derivation path
    // Path: m/44'/118'/0'/0/0
    let derived_key = derive_cosmos_key(&seed, 0, 0, 0)?;
    
    // Convert to hex string
    Ok(hex::encode(derived_key))
}

/// Derives a Cosmos/Celestia private key from a seed
/// 
/// Uses SLIP-10 (ed25519 curve) with the standard Cosmos derivation path
fn derive_cosmos_key(seed: &[u8], account: u32, change: u32, index: u32) -> Result<[u8; 32]> {
    // Cosmos derivation path: m/44'/118'/account'/change/index
    // The ' indicates hardened derivation (0x80000000 | index)
    const HARDENED: u32 = 0x80000000;
    
    let path = vec![
        HARDENED | 44,     // 44' - BIP44 purpose
        HARDENED | 118,    // 118' - Cosmos coin type
        HARDENED | account, // account' - hardened account
        change,             // change - not hardened
        index,              // index - not hardened
    ];
    
    let derived = derive_ed25519_private_key(seed, &path);
    
    Ok(derived)
}

/// Validates that a hex string is a valid private key (32 bytes)
pub fn validate_private_key_hex(hex_str: &str) -> Result<()> {
    let bytes = hex::decode(hex_str)
        .context("Invalid hex string")?;
    
    if bytes.len() != 32 {
        anyhow::bail!("Private key must be exactly 32 bytes (64 hex characters), got {} bytes", bytes.len());
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_private_key_hex() {
        // Valid 32-byte key
        let valid = "393fdb5def075819de55756b45c9e2c8531a8c78dd6eede483d3440e9457d839";
        assert!(validate_private_key_hex(valid).is_ok());
        
        // Too short
        let short = "393fdb5def075819";
        assert!(validate_private_key_hex(short).is_err());
        
        // Invalid hex
        let invalid = "not-hex-string";
        assert!(validate_private_key_hex(invalid).is_err());
    }

    #[test]
    fn test_mnemonic_to_private_key() {
        // Example mnemonic (DO NOT USE IN PRODUCTION)
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        
        let result = mnemonic_to_private_key_hex(mnemonic);
        assert!(result.is_ok());
        
        let hex_key = result.unwrap();
        assert_eq!(hex_key.len(), 64); // 32 bytes = 64 hex chars
        
        // Validate the derived key
        assert!(validate_private_key_hex(&hex_key).is_ok());
    }
}

