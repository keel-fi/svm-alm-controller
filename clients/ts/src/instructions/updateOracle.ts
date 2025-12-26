import { Address, TransactionSigner, OptionOrNullable } from "@solana/kit";
import { getUpdateOracleInstruction } from "../generated/instructions/updateOracle";
import { deriveControllerAuthorityPda } from "../pdas";
import { SVM_ALM_CONTROLLER_PROGRAM_ADDRESS } from "../generated";
import type { FeedArgs } from "../generated/types";

/**
 * Instruction generation for updating an oracle account
 */
export async function createUpdateOracleInstruction(
  controller: Address,
  authority: TransactionSigner,
  oracle: Address,
  priceFeed: Address,
  feedArgs: OptionOrNullable<FeedArgs>,
  newAuthority?: Address
) {
  const controllerAuthority = await deriveControllerAuthorityPda(controller);

  return getUpdateOracleInstruction({
    controller,
    controllerAuthority,
    authority,
    oracle,
    priceFeed,
    newAuthority,
    feedArgs,
  });
}

