import { Address, TransactionSigner, AccountMeta, AccountRole } from "@solana/kit";
import { getInitializeIntegrationInstruction } from "../../generated/instructions/initializeIntegration";
import {
  deriveControllerAuthorityPda,
  deriveIntegrationPda,
  derivePermissionPda,
} from "../../pdas";
import { SVM_ALM_CONTROLLER_PROGRAM_ADDRESS } from "../../generated";
import type {
  IntegrationStatusArgs,
  InitializeArgsArgs,
  DriftConfigArgs,
  IntegrationConfigArgs,
} from "../../generated/types";
import { IntegrationType } from "../../generated/types";
import { computeIntegrationHash } from "../utils";
import {
  deriveStatePda,
  deriveUserStatsPda,
  deriveUserPda,
  deriveSpotMarketPda,
  DRIFT_PROGRAM_ID,
} from "../../integrations/drift/pdas";

const SYSTEM_PROGRAM_ID = "11111111111111111111111111111111" as Address;
const RENT_SYSVAR_ID = "SysvarRent111111111111111111111111111111111" as Address;

/**
 * Instruction generation for initializing Drift integration
 * 
 * NOTE: This function requires borsh serialization and keccak hashing libraries
 * to compute the integration hash. See computeIntegrationHash in utils.ts
 */
export async function createDriftInitializeIntegrationInstruction(
  payer: TransactionSigner,
  controller: Address,
  authority: TransactionSigner,
  mint: Address,
  description: string,
  status: IntegrationStatusArgs,
  rateLimitSlope: number | bigint,
  rateLimitMaxOutflow: number | bigint,
  permitLiquidation: boolean,
  subAccountId: number,
  spotMarketIndex: number,
  poolId: number
) {
  const config: IntegrationConfigArgs = {
    __kind: "Drift",
    fields: [
      {
        subAccountId,
        spotMarketIndex,
        poolId,
        padding: new Uint8Array(219),
      } as DriftConfigArgs,
    ],
  };

  const innerArgs: InitializeArgsArgs = {
    __kind: "Drift",
    subAccountId,
    spotMarketIndex,
  };

  // Hash the config to derive the integration PDA
  const configHash = computeIntegrationHash(config);
  const integrationPda = await deriveIntegrationPda(controller, configHash);
  const permissionPda = await derivePermissionPda(controller, authority.address);
  const controllerAuthority = await deriveControllerAuthorityPda(controller);

  // Encode description to 32 bytes
  const descriptionBytes = new TextEncoder().encode(description);
  if (descriptionBytes.length > 32) {
    console.warn(`Description exceeds 32 bytes (${descriptionBytes.length}), truncating`);
  }
  const descriptionEncoding = new Uint8Array(32);
  descriptionEncoding.set(descriptionBytes.slice(0, 32));

  // Derive Drift PDAs
  const userStats = await deriveUserStatsPda(controllerAuthority);
  const user = await deriveUserPda(controllerAuthority, subAccountId);
  const state = await deriveStatePda();
  const spotMarket = await deriveSpotMarketPda(spotMarketIndex);

  const remainingAccounts: AccountMeta[] = [
    {
      address: mint,
      role: AccountRole.READONLY,
    },
    {
      address: user,
      role: AccountRole.WRITABLE,
    },
    {
      address: userStats,
      role: AccountRole.WRITABLE,
    },
    {
      address: state,
      role: AccountRole.WRITABLE,
    },
    {
      address: spotMarket,
      role: AccountRole.READONLY,
    },
    {
      address: RENT_SYSVAR_ID,
      role: AccountRole.READONLY,
    },
    {
      address: DRIFT_PROGRAM_ID,
      role: AccountRole.READONLY,
    },
  ];

  const instruction = getInitializeIntegrationInstruction({
    payer,
    controller,
    controllerAuthority,
    authority,
    permission: permissionPda,
    integration: integrationPda,
    programId: SVM_ALM_CONTROLLER_PROGRAM_ADDRESS,
    systemProgram: SYSTEM_PROGRAM_ID,
    integrationType: IntegrationType.Drift,
    status,
    description: descriptionEncoding,
    rateLimitSlope,
    rateLimitMaxOutflow,
    permitLiquidation,
    innerArgs,
  });

  // Add remaining accounts
  return {
    ...instruction,
    accounts: [...instruction.accounts, ...remainingAccounts],
  };
}

