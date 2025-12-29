import { Address, AccountMeta, Instruction, AccountRole } from "@solana/kit";
import { anchorDiscriminator } from "../integrations";

const KAMINO_LEND_PROGRAM_ID =
  "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD" as Address;

export function createRefreshKaminoObligationInstruction(
  market: Address,
  obligation: Address,
  reserves: Address[]
): Instruction {
  const data = anchorDiscriminator("global", "refresh_obligation");

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

