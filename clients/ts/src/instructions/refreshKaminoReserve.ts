import { Address, AccountMeta, Instruction, AccountRole } from "@solana/kit";

const KAMINO_LEND_PROGRAM_ID =
  "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD" as Address;

/**
 * Anchor discriminator for refresh_reserve instruction
 * This is the first 8 bytes of: sha256("global:refresh_reserve")[0..8]
 */
function getRefreshReserveDiscriminator(): Uint8Array {
  // Anchor discriminator for "global:refresh_reserve"
  // This should match the actual discriminator used by Kamino
  // For now, using a placeholder - this should be calculated from the actual anchor IDL
  return new Uint8Array([0x7b, 0x2e, 0x8d, 0x4f, 0x9a, 0x1c, 0x5b, 0x6d]);
}

/**
 * Instruction generation for refreshing a Kamino reserve
 */
export function createRefreshKaminoReserveInstruction(
  reserve: Address,
  market: Address,
  scopePrices: Address
): Instruction {
  const data = getRefreshReserveDiscriminator();

  const accounts: AccountMeta[] = [
    {
      address: reserve,
      role: AccountRole.WRITABLE,
    },
    {
      address: market,
      role: AccountRole.READONLY,
    },
    // pyth oracle
    {
      address: KAMINO_LEND_PROGRAM_ID,
      role: AccountRole.READONLY,
    },
    // switchboard_price_oracle
    {
      address: KAMINO_LEND_PROGRAM_ID,
      role: AccountRole.READONLY,
    },
    // switchboard_twap_oracle
    {
      address: KAMINO_LEND_PROGRAM_ID,
      role: AccountRole.READONLY,
    },
    // scope_prices
    {
      address: scopePrices,
      role: AccountRole.READONLY,
    },
  ];

  return {
    programAddress: KAMINO_LEND_PROGRAM_ID,
    accounts,
    data,
  };
}

