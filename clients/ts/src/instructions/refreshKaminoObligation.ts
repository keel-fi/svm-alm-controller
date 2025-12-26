import { Address, AccountMeta, Instruction, AccountRole } from "@solana/kit";

const KAMINO_LEND_PROGRAM_ID =
  "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD" as Address;

/**
 * Anchor discriminator for refresh_obligation instruction
 * This is the first 8 bytes of: sha256("global:refresh_obligation")[0..8]
 * 
 * NOTE: This is a placeholder. The actual discriminator should be calculated
 * from the Kamino Anchor IDL using: sha256("global:refresh_obligation")[0..8]
 */
function getRefreshObligationDiscriminator(): Uint8Array {
  // Anchor discriminator for "global:refresh_obligation"
  // This should match the actual discriminator used by Kamino
  // For now, using a placeholder - this should be calculated from the actual anchor IDL
  return new Uint8Array([0x8a, 0x1f, 0x9c, 0x5e, 0x8f, 0x2d, 0x3a, 0x4b]);
}

/**
 * If obligation has reserves, they need to be added as remaining accounts.
 * For the sake of simplicity, this method only supports obligations with 1 reserve.
 * TODO: add support for more
 */
export function createRefreshKaminoObligationInstruction(
  market: Address,
  obligation: Address,
  reserves: Address[]
): Instruction {
  const data = getRefreshObligationDiscriminator();

  const accounts: AccountMeta[] = [
    {
      address: market,
      role: AccountRole.READONLY,
    },
    {
      address: obligation,
      role: AccountRole.WRITABLE,
    },
    ...reserves.map((reserve) => ({
      address: reserve,
      role: AccountRole.WRITABLE,
    })),
  ];

  return {
    programAddress: KAMINO_LEND_PROGRAM_ID,
    accounts,
    data,
  };
}

