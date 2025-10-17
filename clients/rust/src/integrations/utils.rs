use sha2_const_stable::Sha256;

/// Compute the first 8 bytes of SHA256(namespace:name) in a `const fn`.
///
/// # Arguments
///
/// * `namespace` - The namespace to compute the discriminator for.
/// * `name` - The name to compute the discriminator for.
///
/// # Returns
///
/// The first 8 bytes of the SHA256 hash.
pub const fn anchor_discriminator(namespace: &str, name: &str) -> [u8; 8] {
    let hash = Sha256::new()
        .update(namespace.as_bytes())
        .update(b":")
        .update(name.as_bytes())
        .finalize();

    // return the first 8 bytes as the discriminator
    [
        hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7],
    ]
}