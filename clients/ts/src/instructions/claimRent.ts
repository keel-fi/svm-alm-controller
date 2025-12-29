import { Address, TransactionSigner } from "@solana/kit";
import { getClaimRentInstruction } from "../generated/instructions/claimRent";
import {
  deriveControllerAuthorityPda,
  derivePermissionPda,
} from "../pdas";
import { SVM_ALM_CONTROLLER_PROGRAM_ADDRESS } from "../generated";

/**
 * Instruction generation for claiming rent
 */
export async function createClaimRentInstruction(
  controller: Address,
  authority: TransactionSigner,
  destination: Address
) {
  const controllerAuthority = await deriveControllerAuthorityPda(controller);
  const permission = await derivePermissionPda(controller, authority.address);

  return getClaimRentInstruction(
    {
      controller,
      controllerAuthority,
      authority,
      permission,
      destination,
    },
    {
      programAddress: SVM_ALM_CONTROLLER_PROGRAM_ADDRESS,
    }
  );
}

