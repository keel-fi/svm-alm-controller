import { Address, TransactionSigner } from "@solana/kit";
import { getInitializeReserveInstruction } from "../generated/instructions/initializeReserve";
import {
  deriveControllerAuthorityPda,
  derivePermissionPda,
  deriveReservePda,
} from "../pdas";
import { getAssociatedTokenAddress } from "./utils";
import { SVM_ALM_CONTROLLER_PROGRAM_ADDRESS } from "../generated";
import type { ReserveStatusArgs } from "../generated/types";

const ASSOCIATED_TOKEN_PROGRAM_ID = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL" as Address;

/**
 * Instruction generation for initializing a reserve account
 */
export async function createInitializeReserveInstruction(
  payer: TransactionSigner,
  controller: Address,
  authority: TransactionSigner,
  mint: Address,
  tokenProgram: Address,
  status: ReserveStatusArgs,
  rateLimitSlope: number | bigint,
  rateLimitMaxOutflow: number | bigint
) {
  const permissionPda = await derivePermissionPda(controller, authority.address);
  const reservePda = await deriveReservePda(controller, mint);
  const controllerAuthority = await deriveControllerAuthorityPda(controller);
  const vault = await getAssociatedTokenAddress(
    controllerAuthority,
    mint,
    tokenProgram
  );

  return getInitializeReserveInstruction({
    payer,
    controller,
    controllerAuthority,
    authority,
    permission: permissionPda,
    reserve: reservePda,
    mint,
    vault,
    tokenProgram,
    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
    programId: SVM_ALM_CONTROLLER_PROGRAM_ADDRESS,
    status,
    rateLimitSlope,
    rateLimitMaxOutflow,
  });
}

