import { Address, TransactionSigner } from "@solana/kit";
import { getSyncInstruction } from "../generated/instructions/sync";
import { deriveControllerAuthorityPda } from "../pdas";
import { SVM_ALM_CONTROLLER_PROGRAM_ADDRESS } from "../generated";

/**
 * Instruction generation for syncing an integration account
 */
export async function createSyncIntegrationInstruction(
  controller: Address,
  payer: TransactionSigner,
  integration: Address,
  reserve: Address
) {
  const controllerAuthority = await deriveControllerAuthorityPda(controller);

  return getSyncInstruction({
    controller,
    controllerAuthority,
    payer,
    integration,
    reserve,
  });
}

