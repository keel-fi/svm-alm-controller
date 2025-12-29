import { Address, AccountMeta, Instruction, AccountRole } from "@solana/kit";
import { anchorDiscriminator, kamino } from "../integrations";

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
    programAddress: kamino.KAMINO_LEND_PROGRAM_ID,
    accounts,
    data,
  };
}

