import { Address, TransactionSigner } from "@solana/kit";
import { getManageControllerInstruction } from "../generated/instructions/manageController";
import {
  deriveControllerAuthorityPda,
  derivePermissionPda,
} from "../pdas";
import { SVM_ALM_CONTROLLER_PROGRAM_ADDRESS } from "../generated";
import type { ControllerStatusArgs } from "../generated/types";

/**
 * Instruction generation for managing a controller account
 */
export async function createManageControllerInstruction(
  controller: Address,
  authority: TransactionSigner,
  status: ControllerStatusArgs
) {
  const permissionPda = await derivePermissionPda(controller, authority.address);
  const controllerAuthority = await deriveControllerAuthorityPda(controller);

  return getManageControllerInstruction({
    controller,
    controllerAuthority,
    authority,
    permission: permissionPda,
    programId: SVM_ALM_CONTROLLER_PROGRAM_ADDRESS,
    status,
  });
}

