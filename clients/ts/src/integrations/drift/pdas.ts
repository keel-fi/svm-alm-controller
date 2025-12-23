import {
  address,
  Address,
  getAddressEncoder,
  getU16Encoder,
  getProgramDerivedAddress,
} from "@solana/addresses";

export const DRIFT_PROGRAM_ID =
  "dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH" as Address;

/// Derives Drift CPI signer
export const deriveDriftSigner = async () => {
  const [driftSignerPda] = await getProgramDerivedAddress({
    programAddress: DRIFT_PROGRAM_ID,
    seeds: ["drift_signer"],
  });

  return driftSignerPda;
};

/// Derives Drift CPI signer nonce
export const deriveDriftSignerNonce = async (): Promise<number> => {
  const [, nonce] = await getProgramDerivedAddress({
    programAddress: DRIFT_PROGRAM_ID,
    seeds: ["drift_signer"],
  });

  return nonce;
};

/// Derives State PDA
export const deriveStatePda = async () => {
  const [statePda] = await getProgramDerivedAddress({
    programAddress: DRIFT_PROGRAM_ID,
    seeds: ["drift_state"],
  });

  return statePda;
};

/// Derives UserStats PDA
export const deriveUserStatsPda = async (authority: Address) => {
  const addressEncoder = getAddressEncoder();

  const [userStatsPda] = await getProgramDerivedAddress({
    programAddress: DRIFT_PROGRAM_ID,
    seeds: ["user_stats", addressEncoder.encode(authority)],
  });

  return userStatsPda;
};

/// Derives User subaccount PDA
export const deriveUserPda = async (
  authority: Address,
  subAccountId: number
) => {
  const addressEncoder = getAddressEncoder();

  const [userPda] = await getProgramDerivedAddress({
    programAddress: DRIFT_PROGRAM_ID,
    seeds: [
      "user",
      addressEncoder.encode(authority),
      getU16Encoder
      .encode(subAccountId),
    ],
  });

  return userPda;
};

/// Derives SpotMarket PDA
export const deriveSpotMarketPda = async (marketIndex: number) => {
  const [spotMarketPda] = await getProgramDerivedAddress({
    programAddress: DRIFT_PROGRAM_ID,
    seeds: [
      "spot_market",
      getU16Encoder.encode(marketIndex),
    ],
  });

  return spotMarketPda;
};

/// Derives SpotMarket Vault PDA
export const deriveSpotMarketVaultPda = async (marketIndex: number) => {
  const [spotMarketVaultPda] = await getProgramDerivedAddress({
    programAddress: DRIFT_PROGRAM_ID,
    seeds: [
      "spot_market_vault",
      getU16Encoder.encode(marketIndex),
    ],
  });

  return spotMarketVaultPda;
};
