use crate::error::{AuthCoreError, Result};
use crate::origin::ServerOrigin;
use crate::schemas::SessionTokensV3;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use p256::ecdsa::{signature::Signer, Signature, SigningKey};
use p256::elliptic_curve::sec1::ToEncodedPoint;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;
use zeroize::Zeroize;

/// Explicit honesty about key provenance.
/// Software or OS-keychain-backed keys MUST NEVER report `HardwareBackedAttested`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceAttestationLevel {
    SoftwareUnattested,
    OsKeychainUnattested,
    HardwareBackedAttested,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EcPublicJwk {
    pub kty: String,
    pub crv: String,
    pub x: String,
    pub y: String,
}

/// Platform adapter boundary for per-device P-256 signing keys.
/// OS adapters in `atlas-commerce-desktop` implement this trait backed by
/// OS keychain / TPM / Secure Enclave without exposing private key material
/// to `atlas-auth-core` logs or errors.
pub trait DeviceKeyAdapter {
    fn public_jwk(&self) -> EcPublicJwk;
    fn sign_es256(&self, signing_input: &[u8]) -> Result<[u8; 64]>;
    fn attestation_level(&self) -> DeviceAttestationLevel;

    fn thumbprint(&self) -> String {
        jwk_thumbprint(&self.public_jwk())
    }
}

/// Reference in-memory / software P-256 adapter.
/// Always reports `DeviceAttestationLevel::SoftwareUnattested`.
pub struct SoftwareP256Key {
    signing_key: SigningKey,
}

impl SoftwareP256Key {
    pub fn generate() -> Result<Self> {
        let mut seed = [0u8; 32];
        loop {
            getrandom::getrandom(&mut seed).map_err(|_| AuthCoreError::CryptoFailure)?;
            if let Ok(key) = SigningKey::from_bytes((&seed).into()) {
                seed.zeroize();
                return Ok(Self { signing_key: key });
            }
        }
    }

    pub fn from_pkcs8_bytes(pkcs8_der: &[u8]) -> Result<Self> {
        use p256::pkcs8::DecodePrivateKey;
        let signing_key =
            SigningKey::from_pkcs8_der(pkcs8_der).map_err(|_| AuthCoreError::CryptoFailure)?;
        Ok(Self { signing_key })
    }

    pub fn to_pkcs8_bytes(&self) -> Result<zeroize::Zeroizing<Vec<u8>>> {
        use p256::pkcs8::EncodePrivateKey;
        let doc = self
            .signing_key
            .to_pkcs8_der()
            .map_err(|_| AuthCoreError::CryptoFailure)?;
        Ok(zeroize::Zeroizing::new(doc.as_bytes().to_vec()))
    }
}

impl std::fmt::Debug for SoftwareP256Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoftwareP256Key")
            .field("thumbprint", &self.thumbprint())
            .field("attestation", &self.attestation_level())
            .finish()
    }
}

impl DeviceKeyAdapter for SoftwareP256Key {
    fn public_jwk(&self) -> EcPublicJwk {
        let verifying = self.signing_key.verifying_key();
        let point = verifying.to_encoded_point(false);
        let x = URL_SAFE_NO_PAD.encode(point.x().expect("uncompressed P-256 point has x"));
        let y = URL_SAFE_NO_PAD.encode(point.y().expect("uncompressed P-256 point has y"));
        EcPublicJwk {
            kty: "EC".to_string(),
            crv: "P-256".to_string(),
            x,
            y,
        }
    }

    fn sign_es256(&self, signing_input: &[u8]) -> Result<[u8; 64]> {
        let sig: Signature = self.signing_key.sign(signing_input);
        let bytes = sig.to_bytes();
        let mut out = [0u8; 64];
        out.copy_from_slice(&bytes);
        Ok(out)
    }

    fn attestation_level(&self) -> DeviceAttestationLevel {
        DeviceAttestationLevel::SoftwareUnattested
    }
}

/// RFC 7638 canonical JWK thumbprint for EC P-256 keys (base64url SHA-256).
/// Matches `worker/lib/desktop-device-proof.ts` `jwkThumbprint()` byte-for-byte.
pub fn jwk_thumbprint(jwk: &EcPublicJwk) -> String {
    let canonical = format!(
        r#"{{"crv":"{}","kty":"{}","x":"{}","y":"{}"}}"#,
        jwk.crv, jwk.kty, jwk.x, jwk.y
    );
    URL_SAFE_NO_PAD.encode(Sha256::digest(canonical.as_bytes()))
}

#[derive(Serialize)]
struct DpopHeader {
    typ: &'static str,
    alg: &'static str,
    jwk: EcPublicJwk,
}

#[derive(Serialize)]
struct DpopClaims<'a> {
    jti: &'a str,
    htm: &'a str,
    htu: &'a str,
    iat: u64,
    nonce: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    ath: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    atlas_txn: Option<&'a str>,
}

/// Builds a fresh RFC 9449 `dpop+jwt` ES256 proof matching `worker/lib/desktop-device-proof.ts`.
pub fn build_dpop_proof(
    key: &dyn DeviceKeyAdapter,
    method: &str,
    target_url: &str,
    nonce: &str,
    iat_secs: u64,
    access_token: Option<&str>,
    atlas_txn: Option<&str>,
) -> Result<String> {
    if !method.bytes().all(|b| b.is_ascii_uppercase()) || method.is_empty() {
        return Err(AuthCoreError::InvalidTargetUrl);
    }
    if nonce.len() != 43
        || !nonce
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(AuthCoreError::InvalidNonce);
    }
    let parsed = Url::parse(target_url).map_err(|_| AuthCoreError::InvalidTargetUrl)?;
    let origin = ServerOrigin::parse(&parsed.origin().ascii_serialization())?;
    if parsed.fragment().is_some() {
        return Err(AuthCoreError::InvalidTargetUrl);
    }
    let htu = origin.endpoint_url(parsed.path())?;

    let mut jti_bytes = [0u8; 24];
    getrandom::getrandom(&mut jti_bytes).map_err(|_| AuthCoreError::CryptoFailure)?;
    let jti = URL_SAFE_NO_PAD.encode(jti_bytes);

    let ath = access_token.map(|tok| {
        let digest = Sha256::digest(tok.as_bytes());
        URL_SAFE_NO_PAD.encode(digest)
    });

    let header = DpopHeader {
        typ: "dpop+jwt",
        alg: "ES256",
        jwk: key.public_jwk(),
    };
    let claims = DpopClaims {
        jti: &jti,
        htm: method,
        htu: &htu,
        iat: iat_secs,
        nonce,
        ath,
        atlas_txn,
    };

    let h_b64 = URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&header).map_err(|_| AuthCoreError::CryptoFailure)?);
    let c_b64 = URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&claims).map_err(|_| AuthCoreError::CryptoFailure)?);
    let signing_input = format!("{}.{}", h_b64, c_b64);
    let sig = key.sign_es256(signing_input.as_bytes())?;
    let s_b64 = URL_SAFE_NO_PAD.encode(sig);

    Ok(format!("{}.{}", signing_input, s_b64))
}

/// Formats the RFC 9449 `Authorization` header required by Atlas Desktop v3
/// protected endpoints: `Authorization: DPoP nv3.<...>`.
pub fn authorization_header_for_token(tokens: &SessionTokensV3) -> Result<String> {
    if tokens.token_type != "DPoP" || !tokens.access_token.starts_with("nv3.") {
        return Err(AuthCoreError::ResponseMismatch);
    }
    Ok(format!("DPoP {}", tokens.access_token))
}

/// Builds both headers required on every Desktop v3 protected resource request:
/// `(Authorization: DPoP nv3.<...>, DPoP: <dpop+jwt bound with ath>)`.
pub fn build_protected_request_headers(
    key: &dyn DeviceKeyAdapter,
    method: &str,
    target_url: &str,
    nonce: &str,
    iat_secs: u64,
    tokens: &SessionTokensV3,
) -> Result<(String, String)> {
    let auth = authorization_header_for_token(tokens)?;
    let dpop = build_dpop_proof(
        key,
        method,
        target_url,
        nonce,
        iat_secs,
        Some(&tokens.access_token),
        None,
    )?;
    Ok((auth, dpop))
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::{signature::Verifier, VerifyingKey};

    #[test]
    fn generates_verifiable_rfc9449_dpop_proof_and_never_claims_hw_attestation() {
        let key = SoftwareP256Key::generate().unwrap();
        assert_eq!(
            key.attestation_level(),
            DeviceAttestationLevel::SoftwareUnattested
        );
        let nonce = "A".repeat(43);
        let jwt = build_dpop_proof(
            &key,
            "POST",
            "https://stock.afmarbre.com/api/auth/desktop/v3/requests",
            &nonce,
            1_760_000_000,
            Some("nv3.exampletoken"),
            Some(&"0".repeat(64)),
        )
        .unwrap();

        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3);

        let hdr: serde_json::Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0]).unwrap()).unwrap();
        assert_eq!(hdr["typ"], "dpop+jwt");
        assert_eq!(hdr["alg"], "ES256");
        assert_eq!(hdr["jwk"]["kty"], "EC");
        assert_eq!(hdr["jwk"]["crv"], "P-256");

        let claims: serde_json::Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
        assert_eq!(claims["htm"], "POST");
        assert_eq!(
            claims["htu"],
            "https://stock.afmarbre.com/api/auth/desktop/v3/requests"
        );
        assert_eq!(claims["nonce"], nonce);
        assert!(claims["ath"].as_str().is_some());

        let sig_bytes = URL_SAFE_NO_PAD.decode(parts[2]).unwrap();
        let sig = Signature::from_slice(&sig_bytes).unwrap();
        let x = URL_SAFE_NO_PAD
            .decode(hdr["jwk"]["x"].as_str().unwrap())
            .unwrap();
        let y = URL_SAFE_NO_PAD
            .decode(hdr["jwk"]["y"].as_str().unwrap())
            .unwrap();
        assert_eq!(x.len(), 32);
        assert_eq!(y.len(), 32);
        let mut sec1 = Vec::with_capacity(65);
        sec1.push(0x04);
        sec1.extend_from_slice(&x);
        sec1.extend_from_slice(&y);
        let vk = VerifyingKey::from_sec1_bytes(&sec1).unwrap();
        let msg = format!("{}.{}", parts[0], parts[1]);
        vk.verify(msg.as_bytes(), &sig).unwrap();

        let query_jwt = build_dpop_proof(
            &key,
            "GET",
            "https://stock.afmarbre.com/api/products?page=2",
            &nonce,
            1_760_000_000,
            None,
            None,
        )
        .unwrap();
        let q_parts: Vec<&str> = query_jwt.split('.').collect();
        let q_claims: serde_json::Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(q_parts[1]).unwrap()).unwrap();
        assert_eq!(q_claims["htu"], "https://stock.afmarbre.com/api/products");
    }
}
