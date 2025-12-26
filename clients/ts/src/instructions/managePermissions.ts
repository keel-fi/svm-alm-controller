import { Address, TransactionSigner } from "@solana/kit";
import { getManagePermissionInstruction } from "../generated/instructions/managePermission";
import {
  deriveControllerAuthorityPda,
  derivePermissionPda,
} from "../pdas";
import { SVM_ALM_CONTROLLER_PROGRAM_ADDRESS } from "../generated";
import type { PermissionStatusArgs } from "../generated/types";

const SYSTEM_PROGRAM_ID = "11111111111111111111111111111111" as Address;

/**
 * Instruction generation for managing a permission account
 */
export async function createManagePermissionsInstruction(
  controller: Address,
  payer: TransactionSigner,
  callingAuthority: TransactionSigner,
  subjectAuthority: Address,
  status: PermissionStatusArgs,
  canExecuteSwap: boolean,
  canManagePermissions: boolean,
  canInvokeExternalTransfer: boolean,
  canReallocate: boolean,
  canFreezeController: boolean,
  canUnfreezeController: boolean,
  canManageReservesAndIntegrations: boolean,
  canSuspendPermissions: boolean,
  canLiquidate: boolean
) {
  const callingPermissionPda = await derivePermissionPda(
    controller,
    callingAuthority.address
  );
  const controllerAuthority = await deriveControllerAuthorityPda(controller);
  const subjectPermissionPda = await derivePermissionPda(
    controller,
    subjectAuthority
  );

  return getManagePermissionInstruction({
    controller,
    controllerAuthority,
    superAuthority: callingAuthority,
    superPermission: callingPermissionPda,
    authority: subjectAuthority,
    permission: subjectPermissionPda,
    payer,
    programId: SVM_ALM_CONTROLLER_PROGRAM_ADDRESS,
    systemProgram: SYSTEM_PROGRAM_ID,
    status,
    canExecuteSwap,
    canManagePermissions,
    canInvokeExternalTransfer,
    canReallocate,
    canFreezeController,
    canUnfreezeController,
    canManageReservesAndIntegrations,
    canSuspendPermissions,
    canLiquidate,
  });
}

