import { Address, TransactionSigner, OptionOrNullable } from "@solana/kit";
import { getManageIntegrationInstruction } from "../generated/instructions/manageIntegration";
import {
  deriveControllerAuthorityPda,
  derivePermissionPda,
} from "../pdas";
import { SVM_ALM_CONTROLLER_PROGRAM_ADDRESS } from "../generated";
import type { IntegrationStatusArgs } from "../generated/types";

/**
 * Instruction generation for managing an integration account
 */
export async function createManageIntegrationInstruction(
  controller: Address,
  authority: TransactionSigner,
  integration: Address,
  status: OptionOrNullable<IntegrationStatusArgs>,
  rateLimitSlope: OptionOrNullable<number | bigint>,
  rateLimitMaxOutflow: OptionOrNullable<number | bigint>,
  description?: OptionOrNullable<Uint8Array>
) {
  const permissionPda = await derivePermissionPda(controller, authority.address);
  const controllerAuthority = await deriveControllerAuthorityPda(controller);

  return getManageIntegrationInstruction({
    controller,
    controllerAuthority,
    authority,
    permission: permissionPda,
    integration,
    programId: SVM_ALM_CONTROLLER_PROGRAM_ADDRESS,
    status,
    description: description ?? null,
    rateLimitSlope,
    rateLimitMaxOutflow,
  });
}

