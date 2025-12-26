import { Address, getAddressEncoder, getProgramDerivedAddress } from "@solana/addresses";
import { address } from "@solana/addresses";
import createKeccakHash from "keccak";
import { getIntegrationConfigEncoder, IntegrationConfigArgs } from "../generated";

const ASSOCIATED_TOKEN_PROGRAM_ID = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL" as Address;

/**
 * Derives the associated token address for a given owner and mint
 */
export async function getAssociatedTokenAddress(
  owner: Address,
  mint: Address,
  tokenProgram: Address = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA" as Address
): Promise<Address> {
  const addressEncoder = getAddressEncoder();
  
  const [ata] = await getProgramDerivedAddress({
    programAddress: address(ASSOCIATED_TOKEN_PROGRAM_ID),
    seeds: [
      addressEncoder.encode(owner),
      addressEncoder.encode(tokenProgram),
      addressEncoder.encode(mint),
    ],
  });

  return ata;
}

/**
 * Compute integration hash from integration type and config
 * @param integrationType
 * @param config
 * @returns
 */
export const computeIntegrationHash = (
    config: IntegrationConfigArgs
  ): Uint8Array => {
    let ixBytes: Buffer;
    let hash: Uint8Array;
  
    ixBytes = Buffer.from(getIntegrationConfigEncoder().encode(config));
    hash = createKeccakHash("keccak256").update(ixBytes).digest();
    return hash;
  };
