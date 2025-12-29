import { Address, AccountMeta, Instruction, AccountRole } from "@solana/kit";
import { anchorDiscriminator } from "../integrations";

const KAMINO_LEND_PROGRAM_ID =
  "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD" as Address;

/**
 * Instruction generation for refreshing a Kamino reserve
 */
export function createRefreshKaminoReserveInstruction(
  reserve: Address,
  market: Address,
  scopePrices: Address
): Instruction {
  const data = anchorDiscriminator("global", "refresh_reserve");

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

