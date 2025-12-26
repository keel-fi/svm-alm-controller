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
  AtomicSwapConfigArgs,
  IntegrationConfigArgs,
} from "../../generated/types";
import { IntegrationType } from "../../generated/types";
import { computeIntegrationHash } from "../utils";

const SYSTEM_PROGRAM_ID = "11111111111111111111111111111111" as Address;

/**
 * Instruction generation for initializing AtomicSwap integration
 * 
 * NOTE: This function requires borsh serialization and keccak hashing libraries
 * to compute the integration hash. See computeIntegrationHash in utils.ts
 */
export async function createAtomicSwapInitializeIntegrationInstruction(
  payer: TransactionSigner,
  controller: Address,
  authority: TransactionSigner,
  description: string,
  status: IntegrationStatusArgs,
  rateLimitSlope: number | bigint,
  rateLimitMaxOutflow: number | bigint,
  permitLiquidation: boolean,
  inputToken: Address,
  inputMintDecimals: number,
  outputToken: Address,
  outputMintDecimals: number,
  oracle: Address,
  maxStaleness: number | bigint,
  expiryTimestamp: number | bigint,
  maxSlippageBps: number,
  oraclePriceInverted: boolean
) {
  const config: IntegrationConfigArgs = {
    __kind: "AtomicSwap",
    fields: [
      {
        inputToken,
        outputToken,
        oracle,
        maxStaleness,
        expiryTimestamp,
        maxSlippageBps,
        inputMintDecimals,
        outputMintDecimals,
        oraclePriceInverted,
        padding: new Uint8Array(107),
      } as AtomicSwapConfigArgs,
    ],
  };

  const innerArgs: InitializeArgsArgs = {
    __kind: "AtomicSwap",
    maxSlippageBps,
    maxStaleness,
    expiryTimestamp,
    oraclePriceInverted,
  };

  // Hash the config to derive the integration PDA
  const configHash = computeIntegrationHash(config);
  const integrationPda = await deriveIntegrationPda(controller, configHash);
  const permissionPda = await derivePermissionPda(controller, authority.address);
  const controllerAuthority = await deriveControllerAuthorityPda(controller);

  // Encode description to 32 bytes
  const descriptionBytes = new TextEncoder().encode(description);
  const descriptionEncoding = new Uint8Array(32);
  descriptionEncoding.set(descriptionBytes.slice(0, 32));

  const remainingAccounts: AccountMeta[] = [
    {
      address: inputToken,
      role: AccountRole.READONLY,
    },
    {
      address: outputToken,
      role: AccountRole.READONLY,
    },
    {
      address: oracle,
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
    integrationType: IntegrationType.AtomicSwap,
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

