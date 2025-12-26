import { Address } from "@solana/kit";
import { getSyncReserveInstruction } from "../generated/instructions/syncReserve";
import { deriveControllerAuthorityPda, deriveReservePda } from "../pdas";
import { getAssociatedTokenAddress } from "./utils";
import { SVM_ALM_CONTROLLER_PROGRAM_ADDRESS } from "../generated";

/**
 * Instruction generation for syncing a reserve account
 */
export async function createSyncReserveInstruction(
  controller: Address,
  mint: Address,
  tokenProgram: Address
) {
  const reservePda = await deriveReservePda(controller, mint);
  const controllerAuthority = await deriveControllerAuthorityPda(controller);
  const vault = await getAssociatedTokenAddress(
    controllerAuthority,
    mint,
    tokenProgram
  );

  return getSyncReserveInstruction({
    controller,
    controllerAuthority,
    reserve: reservePda,
    vault,
  });
}

