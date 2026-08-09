use ed25519_dalek::{
    SecretKey, Signature, Signer, SigningKey, Verifier, VerifyingKey, PUBLIC_KEY_LENGTH,
    SECRET_KEY_LENGTH, SIGNATURE_LENGTH,
};
use hex;
use rand::rngs::OsRng;
use rand::RngCore;

/// Generate a new Ed25519 keypair
pub fn generate_keypair() -> (SigningKey, VerifyingKey) {
    let mut csprng = OsRng;
    let mut seed = [0u8; 32];
    csprng.fill_bytes(&mut seed);
    // Use the seed directly as the signing key (new in ed25519-dalek 2.x)
    let signing_key = SigningKey::from(seed);
    let verifying_key = signing_key.verifying_key();
    (signing_key, verifying_key)
}

/// Sign a message and return the signature as hex string
pub fn sign(signing_key: &SigningKey, message: &[u8]) -> String {
    let signature = signing_key.sign(message);
    hex::encode(signature.to_bytes())
}

/// Verify a signature (hex string) against a public key (hex string) and message
pub fn verify(public_key_hex: &str, message: &[u8], signature_hex: &str) -> bool {
    let pk_bytes = match hex::decode(public_key_hex) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let sig_bytes = match hex::decode(signature_hex) {
        Ok(b) => b,
        Err(_) => return false,
    };

    let verifying_key = match VerifyingKey::try_from(pk_bytes.as_slice()) {
        Ok(vk) => vk,
        Err(_) => return false,
    };
    let signature = match Signature::try_from(sig_bytes.as_slice()) {
        Ok(s) => s,
        Err(_) => return false,
    };

    verifying_key.verify(message, &signature).is_ok()
}

/// Convert verifying key to hex string (public key)
pub fn public_key_to_hex(vk: &VerifyingKey) -> String {
    hex::encode(vk.as_bytes())
}
