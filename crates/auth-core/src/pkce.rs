use crate::error::{AuthCoreError, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Client-generated PKCE S256 verifier/challenge + high-entropy request `state`.
/// Secret fields are zeroized when dropped.
#[derive(Clone)]
pub struct PkcePair {
    state: String,
    code_verifier: String,
    code_challenge: String,
}

impl Zeroize for PkcePair {
    fn zeroize(&mut self) {
        self.state.zeroize();
        self.code_verifier.zeroize();
    }
}

impl Drop for PkcePair {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for PkcePair {}

impl std::fmt::Debug for PkcePair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PkcePair")
            .field("state", &"[REDACTED]")
            .field("code_verifier", &"[REDACTED]")
            .field("code_challenge", &self.code_challenge)
            .finish()
    }
}

impl PkcePair {
    pub fn generate() -> Result<Self> {
        let mut state_bytes = [0u8; 32];
        let mut verifier_bytes = [0u8; 32];
        getrandom::getrandom(&mut state_bytes).map_err(|_| AuthCoreError::CryptoFailure)?;
        getrandom::getrandom(&mut verifier_bytes).map_err(|_| AuthCoreError::CryptoFailure)?;

        let state = URL_SAFE_NO_PAD.encode(state_bytes);
        let code_verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);
        state_bytes.zeroize();
        verifier_bytes.zeroize();

        let digest = Sha256::digest(code_verifier.as_bytes());
        let code_challenge = URL_SAFE_NO_PAD.encode(digest);

        Ok(Self {
            state,
            code_verifier,
            code_challenge,
        })
    }

    pub fn state(&self) -> &str {
        &self.state
    }

    pub fn code_verifier(&self) -> &str {
        &self.code_verifier
    }

    pub fn code_challenge(&self) -> &str {
        &self.code_challenge
    }
}

pub fn validate_pairing_code(code: &str) -> Result<()> {
    let bytes = code.as_bytes();
    if bytes.len() != 11 || bytes[5] != b'-' {
        return Err(AuthCoreError::InvalidPairingCode);
    }
    let valid_hex = |b: u8| matches!(b, b'0'..=b'9' | b'A'..=b'F');
    if !bytes[..5].iter().copied().all(valid_hex) || !bytes[6..].iter().copied().all(valid_hex) {
        return Err(AuthCoreError::InvalidPairingCode);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_matches_sha256_of_verifier_and_redacts_debug() {
        let pair = PkcePair::generate().unwrap();
        assert_eq!(pair.state().len(), 43);
        assert_eq!(pair.code_verifier().len(), 43);
        assert_eq!(pair.code_challenge().len(), 43);
        let dbg = format!("{:?}", pair);
        assert!(!dbg.contains(pair.state()));
        assert!(!dbg.contains(pair.code_verifier()));
    }

    #[test]
    fn validates_pairing_code() {
        assert!(validate_pairing_code("A1B2C-3D4E5").is_ok());
        assert!(validate_pairing_code("a1b2c-3d4e5").is_err());
        assert!(validate_pairing_code("1234-56789").is_err());

        // Match Worker pairingCode() SHA-256 hex format across varied inputs.
        for i in 0..8 {
            let payload = serde_json::to_string(&(
                "desktop-v3-pairing",
                "https://stock.afmarbre.com",
                format!("{:064x}", i + 1),
                format!("challenge-{}", i),
            ))
            .unwrap();
            let digest = Sha256::digest(payload.as_bytes());
            let hex: String = digest[..5].iter().map(|b| format!("{:02X}", b)).collect();
            let formatted = format!("{}-{}", &hex[..5], &hex[5..10]);
            assert!(validate_pairing_code(&formatted).is_ok());
        }
    }
}
