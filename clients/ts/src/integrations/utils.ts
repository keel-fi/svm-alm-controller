import { createHash } from "crypto";

/**
 * Compute the first 8 bytes of SHA256(namespace:name).
 *
 * @param namespace - The namespace to compute the discriminator for.
 * @param name - The name to compute the discriminator for.
 * @returns The first 8 bytes of the SHA256 hash as a Uint8Array.
 */
export function anchorDiscriminator(
  namespace: string,
  name: string
): Uint8Array {
  const hash = createHash("sha256")
    .update(namespace)
    .update(":")
    .update(name)
    .digest();

  // Return the first 8 bytes as the discriminator
  return hash.subarray(0, 8);
}

