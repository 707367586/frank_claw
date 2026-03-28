//! Ed25519 signature verification for Skill signing (L10 security).
//!
//! Provides signing, verification, and keypair generation using Ed25519.

use clawx_types::ClawxError;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

/// Verify an Ed25519 signature over the SHA-256 hash of `wasm_bytes`.
///
/// Returns `Ok(true)` if valid, `Ok(false)` if the signature does not match,
/// or `Err` if inputs are malformed (bad hex, wrong length).
pub fn verify_skill_signature(
    wasm_bytes: &[u8],
    signature_hex: &str,
    public_key_hex: &str,
) -> clawx_types::Result<bool> {
    let sig_bytes = hex::decode(signature_hex)
        .map_err(|e| ClawxError::Validation(format!("invalid signature hex: {e}")))?;
    let pk_bytes = hex::decode(public_key_hex)
        .map_err(|e| ClawxError::Validation(format!("invalid public key hex: {e}")))?;

    let pk_array: [u8; 32] = pk_bytes
        .try_into()
        .map_err(|_| ClawxError::Validation("public key must be 32 bytes".into()))?;

    let sig_array: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| ClawxError::Validation("signature must be 64 bytes".into()))?;

    let verifying_key = VerifyingKey::from_bytes(&pk_array)
        .map_err(|e| ClawxError::Validation(format!("invalid public key: {e}")))?;

    let signature = Signature::from_bytes(&sig_array);

    let digest = Sha256::digest(wasm_bytes);

    match verifying_key.verify(&digest, &signature) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Sign the SHA-256 hash of `wasm_bytes` with the given Ed25519 secret key.
///
/// `secret_key_hex` is the 32-byte seed encoded as hex (64 hex chars).
/// Returns the 64-byte signature as hex (128 hex chars).
pub fn sign_skill(wasm_bytes: &[u8], secret_key_hex: &str) -> clawx_types::Result<String> {
    let sk_bytes = hex::decode(secret_key_hex)
        .map_err(|e| ClawxError::Validation(format!("invalid secret key hex: {e}")))?;

    let sk_array: [u8; 32] = sk_bytes
        .try_into()
        .map_err(|_| ClawxError::Validation("secret key must be 32 bytes".into()))?;

    let signing_key = SigningKey::from_bytes(&sk_array);

    let digest = Sha256::digest(wasm_bytes);
    let signature = signing_key.sign(&digest);

    Ok(hex::encode(signature.to_bytes()))
}

/// Generate a new Ed25519 keypair.
///
/// Returns `(public_key_hex, secret_key_hex)` where both are 64 hex chars (32 bytes).
pub fn generate_keypair() -> (String, String) {
    let mut rng = rand::thread_rng();
    let signing_key = SigningKey::generate(&mut rng);
    let verifying_key = signing_key.verifying_key();

    let public_key_hex = hex::encode(verifying_key.to_bytes());
    let secret_key_hex = hex::encode(signing_key.to_bytes());

    (public_key_hex, secret_key_hex)
}
