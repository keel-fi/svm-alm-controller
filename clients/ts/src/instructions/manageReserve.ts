import { Address, TransactionSigner } from "@solana/kit";
import { getManageReserveInstruction } from "../generated/instructions/manageReserve";
import {
  deriveControllerAuthorityPda,
  derivePermissionPda,
  deriveReservePda,
} from "../pdas";
import { SVM_ALM_CONTROLLER_PROGRAM_ADDRESS } from "../generated";
import type { ReserveStatusArgs } from "../generated/types";

/**
 * Instruction generation for managing a reserve account
 */
export async function createManageReserveInstruction(
  controller: Address,
  authority: TransactionSigner,
  mint: Address,
  status: ReserveStatusArgs,
  rateLimitSlope: number | bigint,
  rateLimitMaxOutflow: number | bigint
) {
  const permissionPda = await derivePermissionPda(controller, authority.address);
  const reservePda = await deriveReservePda(controller, mint);
  const controllerAuthority = await deriveControllerAuthorityPda(controller);

  return getManageReserveInstruction({
    controller,
    controllerAuthority,
    authority,
    permission: permissionPda,
    reserve: reservePda,
    programId: SVM_ALM_CONTROLLER_PROGRAM_ADDRESS,
    status,
    rateLimitSlope,
    rateLimitMaxOutflow,
  });
}

