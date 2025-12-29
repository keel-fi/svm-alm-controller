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
  KaminoConfigArgs,
  IntegrationConfigArgs,
} from "../../generated/types";
import { IntegrationType } from "../../generated/types";
import { computeIntegrationHash } from "../utils";
import {
  deriveUserMetadataAddress,
  deriveObligationFarmAddress,
  deriveMarketAuthorityAddress,
  KAMINO_LEND_PROGRAM_ID,
  KAMINO_FARMS_PROGRAM_ID,
} from "../../integrations/kamino/pdas";

const SYSTEM_PROGRAM_ID = "11111111111111111111111111111111" as Address;
const RENT_SYSVAR_ID = "SysvarRent111111111111111111111111111111111" as Address;

/**
 * Instruction generation for initializing Kamino Lend integration
 * 
 * NOTE: This function requires borsh serialization and keccak hashing libraries
 * to compute the integration hash. See computeIntegrationHash in utils.ts
 */
export async function createKaminoLendInitializeIntegrationInstruction(
  payer: TransactionSigner,
  controller: Address,
  authority: TransactionSigner,
  description: string,
  status: IntegrationStatusArgs,
  rateLimitSlope: number | bigint,
  rateLimitMaxOutflow: number | bigint,
  permitLiquidation: boolean,
  market: Address,
  reserve: Address,
  reserveLiquidityMint: Address,
  obligation: Address,
  obligationId: number,
  reserveFarmCollateral: Address,
  referrer: Address
) {
  const config: IntegrationConfigArgs = {
    __kind: "Kamino",
    fields: [
      {
        market,
        reserve,
        reserveLiquidityMint,
        obligation,
        obligationId,
        padding: new Uint8Array(95),
      } as KaminoConfigArgs,
    ],
  };

  const innerArgs: InitializeArgsArgs = {
    __kind: "KaminoIntegration",
    obligationId,
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

  // Derive Kamino PDAs
  const { address: userMetadata } = await deriveUserMetadataAddress(controllerAuthority);
  const obligationFarmCollateral = await deriveObligationFarmAddress(
    reserveFarmCollateral,
    obligation
  );
  const { address: marketAuthority } = await deriveMarketAuthorityAddress(market);

  const remainingAccounts: AccountMeta[] = [
    {
      address: obligation,
      role: AccountRole.WRITABLE,
    },
    {
      address: reserveLiquidityMint,
      role: AccountRole.READONLY,
    },
    {
      address: userMetadata,
      role: AccountRole.WRITABLE,
    },
    {
      address: referrer,
      role: AccountRole.READONLY,
    },
    {
      address: obligationFarmCollateral,
      role: AccountRole.WRITABLE,
    },
    {
      address: reserve,
      role: AccountRole.WRITABLE,
    },
    {
      address: reserveFarmCollateral,
      role: AccountRole.WRITABLE,
    },
    {
      address: marketAuthority,
      role: AccountRole.READONLY,
    },
    {
      address: market,
      role: AccountRole.READONLY,
    },
    {
      address: KAMINO_LEND_PROGRAM_ID,
      role: AccountRole.READONLY,
    },
    {
      address: KAMINO_FARMS_PROGRAM_ID,
      role: AccountRole.READONLY,
    },
    {
      address: SYSTEM_PROGRAM_ID,
      role: AccountRole.READONLY,
    },
    {
      address: RENT_SYSVAR_ID,
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
    integrationType: IntegrationType.Kamino,
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

