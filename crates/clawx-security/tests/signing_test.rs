use clawx_security::signing::{generate_keypair, sign_skill, verify_skill_signature};

#[test]
fn test_valid_signature_passes_verification() {
    let (public_key_hex, secret_key_hex) = generate_keypair();
    let wasm_bytes = b"fake wasm module content for testing";

    let signature_hex = sign_skill(wasm_bytes, &secret_key_hex).expect("signing should succeed");
    let result =
        verify_skill_signature(wasm_bytes, &signature_hex, &public_key_hex).expect("verify ok");

    assert!(result, "valid signature must pass verification");
}

#[test]
fn test_tampered_data_fails_verification() {
    let (public_key_hex, secret_key_hex) = generate_keypair();
    let wasm_bytes = b"original wasm content";
    let tampered_bytes = b"tampered wasm content";

    let signature_hex = sign_skill(wasm_bytes, &secret_key_hex).expect("signing should succeed");
    let result = verify_skill_signature(tampered_bytes, &signature_hex, &public_key_hex)
        .expect("verify call should not error");

    assert!(!result, "tampered data must fail verification");
}

#[test]
fn test_wrong_key_fails_verification() {
    let (_pub1, secret1) = generate_keypair();
    let (pub2, _secret2) = generate_keypair();
    let wasm_bytes = b"some wasm bytes";

    let signature_hex = sign_skill(wasm_bytes, &secret1).expect("signing should succeed");
    let result = verify_skill_signature(wasm_bytes, &signature_hex, &pub2)
        .expect("verify call should not error");

    assert!(!result, "wrong public key must fail verification");
}

#[test]
fn test_invalid_hex_signature_returns_error() {
    let (public_key_hex, _) = generate_keypair();
    let wasm_bytes = b"data";

    let result = verify_skill_signature(wasm_bytes, "not-valid-hex!!", &public_key_hex);
    assert!(result.is_err(), "invalid hex signature should return Err");
}

#[test]
fn test_invalid_hex_public_key_returns_error() {
    let wasm_bytes = b"data";
    // Valid hex for signature but garbage for public key
    let fake_sig = "aa".repeat(64); // 64 bytes = valid ed25519 sig length
    let result = verify_skill_signature(wasm_bytes, &fake_sig, "ZZZZ-not-hex");
    assert!(result.is_err(), "invalid hex public key should return Err");
}

#[test]
fn test_invalid_hex_secret_key_returns_error() {
    let wasm_bytes = b"data";
    let result = sign_skill(wasm_bytes, "not-valid-hex!!");
    assert!(result.is_err(), "invalid hex secret key should return Err");
}

#[test]
fn test_wrong_length_public_key_returns_error() {
    let wasm_bytes = b"data";
    let fake_sig = "aa".repeat(64);
    let short_key = "aabb"; // too short for a 32-byte public key
    let result = verify_skill_signature(wasm_bytes, &fake_sig, short_key);
    assert!(result.is_err(), "wrong-length public key should return Err");
}

#[test]
fn test_wrong_length_secret_key_returns_error() {
    let wasm_bytes = b"data";
    let short_key = "aabb";
    let result = sign_skill(wasm_bytes, short_key);
    assert!(result.is_err(), "wrong-length secret key should return Err");
}

#[test]
fn test_keypair_generation_produces_valid_hex() {
    let (public_key_hex, secret_key_hex) = generate_keypair();

    // Ed25519 public key is 32 bytes = 64 hex chars
    assert_eq!(
        public_key_hex.len(),
        64,
        "public key hex should be 64 chars"
    );
    // Ed25519 secret key (seed) is 32 bytes = 64 hex chars
    assert_eq!(
        secret_key_hex.len(),
        64,
        "secret key hex should be 64 chars"
    );

    // Should be valid hex
    hex::decode(&public_key_hex).expect("public key should be valid hex");
    hex::decode(&secret_key_hex).expect("secret key should be valid hex");
}

#[test]
fn test_keypair_generation_is_unique() {
    let (pub1, sec1) = generate_keypair();
    let (pub2, sec2) = generate_keypair();

    assert_ne!(pub1, pub2, "generated public keys should differ");
    assert_ne!(sec1, sec2, "generated secret keys should differ");
}

#[test]
fn test_sign_then_verify_roundtrip() {
    let (public_key_hex, secret_key_hex) = generate_keypair();

    // Simulate various wasm payloads
    let large = vec![0u8; 10_000];
    let payloads: Vec<&[u8]> = vec![b"", b"x", &large];

    for payload in payloads {
        let sig =
            sign_skill(payload, &secret_key_hex).expect("signing should succeed for any payload");
        let ok = verify_skill_signature(payload, &sig, &public_key_hex)
            .expect("verify should succeed");
        assert!(ok, "roundtrip must pass for payload len={}", payload.len());
    }
}

#[test]
fn test_signature_is_deterministic() {
    let (_, secret_key_hex) = generate_keypair();
    let wasm_bytes = b"deterministic test data";

    let sig1 = sign_skill(wasm_bytes, &secret_key_hex).expect("sign 1");
    let sig2 = sign_skill(wasm_bytes, &secret_key_hex).expect("sign 2");

    assert_eq!(sig1, sig2, "signing same data with same key should be deterministic");
}
