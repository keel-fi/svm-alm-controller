import { Address, TransactionSigner } from "@solana/kit";
import { getInitializeOracleInstruction } from "../generated/instructions/initializeOracle";
import { deriveControllerAuthorityPda, deriveOraclePda } from "../pdas";
import { SVM_ALM_CONTROLLER_PROGRAM_ADDRESS } from "../generated";

const SYSTEM_PROGRAM_ID = "11111111111111111111111111111111" as Address;

/**
 * Instruction generation for initializing an oracle account
 */
export async function createInitializeOracleInstruction(
  controller: Address,
  authority: TransactionSigner,
  nonce: Address,
  priceFeed: Address,
  oracleType: number,
  mint: Address,
  quoteMint: Address
) {
  const controllerAuthority = await deriveControllerAuthorityPda(controller);
  const oraclePda = await deriveOraclePda(nonce);

  return getInitializeOracleInstruction({
    controller,
    controllerAuthority,
    authority,
    oracle: oraclePda,
    priceFeed,
    payer: authority,
    systemProgram: SYSTEM_PROGRAM_ID,
    oracleType,
    nonce,
    baseMint: mint,
    quoteMint,
  });
}

