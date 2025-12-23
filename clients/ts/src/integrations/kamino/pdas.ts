import {
  Address,
  getAddressEncoder,
  getProgramDerivedAddress,
} from "@solana/addresses";

export const KAMINO_LEND_PROGRAM_ID =
  "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD" as Address;

export const KAMINO_FARMS_PROGRAM_ID =
  "FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr" as Address;

const DEFAULT_PUBLIC_KEY = "11111111111111111111111111111111" as Address;

/// Derives vanilla obligation address
export const deriveVanillaObligationAddress = async (
  obligationId: number,
  authority: Address,
  market: Address
) => {
  const addressEncoder = getAddressEncoder();

  const [obligationPda] = await getProgramDerivedAddress({
    programAddress: KAMINO_LEND_PROGRAM_ID,
    seeds: [
      // tag 0 for vanilla obligation
      Buffer.from([0]),
      // id
      Buffer.from(new Uint8Array([obligationId])),
      // user
      addressEncoder.encode(authority),
      // kamino market
      addressEncoder.encode(market),
      // seed 1, for lending obligation is the token
      addressEncoder.encode(DEFAULT_PUBLIC_KEY),
      // seed 2, for lending obligation is the token
      addressEncoder.encode(DEFAULT_PUBLIC_KEY),
    ],
  });

  return obligationPda;
};

/// Derives reserve liquidity supply address
export const deriveReserveLiquiditySupply = async (
  market: Address,
  reserveLiquidityMint: Address
) => {
  const addressEncoder = getAddressEncoder();

  const [pda] = await getProgramDerivedAddress({
    programAddress: KAMINO_LEND_PROGRAM_ID,
    seeds: [
      "reserve_liq_supply",
      addressEncoder.encode(market),
      addressEncoder.encode(reserveLiquidityMint),
    ],
  });

  return pda;
};

/// Derives reserve collateral mint address
export const deriveReserveCollateralMint = async (
  market: Address,
  reserveLiquidityMint: Address
) => {
  const addressEncoder = getAddressEncoder();

  const [pda] = await getProgramDerivedAddress({
    programAddress: KAMINO_LEND_PROGRAM_ID,
    seeds: [
      "reserve_coll_mint",
      addressEncoder.encode(market),
      addressEncoder.encode(reserveLiquidityMint),
    ],
  });

  return pda;
};

/// Derives reserve collateral supply address
export const deriveReserveCollateralSupply = async (
  market: Address,
  reserveLiquidityMint: Address
) => {
  const addressEncoder = getAddressEncoder();

  const [pda] = await getProgramDerivedAddress({
    programAddress: KAMINO_LEND_PROGRAM_ID,
    seeds: [
      "reserve_coll_supply",
      addressEncoder.encode(market),
      addressEncoder.encode(reserveLiquidityMint),
    ],
  });

  return pda;
};

/// Derives market authority address
export const deriveMarketAuthorityAddress = async (market: Address) => {
  const addressEncoder = getAddressEncoder();

  const [pda, bump] = await getProgramDerivedAddress({
    programAddress: KAMINO_LEND_PROGRAM_ID,
    seeds: ["lma", addressEncoder.encode(market)],
  });

  return { address: pda, bump };
};

/// Derives obligation farm address
export const deriveObligationFarmAddress = async (
  reserveFarm: Address,
  obligation: Address
) => {
  const addressEncoder = getAddressEncoder();

  const [pda] = await getProgramDerivedAddress({
    programAddress: KAMINO_FARMS_PROGRAM_ID,
    seeds: [
      "user",
      addressEncoder.encode(reserveFarm),
      addressEncoder.encode(obligation),
    ],
  });

  return pda;
};

/// Derives user metadata address
export const deriveUserMetadataAddress = async (user: Address) => {
  const addressEncoder = getAddressEncoder();

  const [pda, bump] = await getProgramDerivedAddress({
    programAddress: KAMINO_LEND_PROGRAM_ID,
    seeds: ["user_meta", addressEncoder.encode(user)],
  });

  return { address: pda, bump };
};

/// Derives rewards vault address
export const deriveRewardsVault = async (
  farmState: Address,
  rewardsVaultMint: Address
) => {
  const addressEncoder = getAddressEncoder();

  const [pda] = await getProgramDerivedAddress({
    programAddress: KAMINO_FARMS_PROGRAM_ID,
    seeds: [
      "rvault",
      addressEncoder.encode(farmState),
      addressEncoder.encode(rewardsVaultMint),
    ],
  });

  return pda;
};

/// Derives rewards treasury vault address
export const deriveRewardsTreasuryVault = async (
  globalConfig: Address,
  rewardsVaultMint: Address
) => {
  const addressEncoder = getAddressEncoder();

  const [pda] = await getProgramDerivedAddress({
    programAddress: KAMINO_FARMS_PROGRAM_ID,
    seeds: [
      "tvault",
      addressEncoder.encode(globalConfig),
      addressEncoder.encode(rewardsVaultMint),
    ],
  });

  return pda;
};

/// Derives farm vaults authority address
export const deriveFarmVaultsAuthority = async (farmState: Address) => {
  const addressEncoder = getAddressEncoder();

  const [pda, bump] = await getProgramDerivedAddress({
    programAddress: KAMINO_FARMS_PROGRAM_ID,
    seeds: ["authority", addressEncoder.encode(farmState)],
  });

  return { address: pda, bump };
};

/// Derives KFarms treasury vault authority address
export const deriveKFarmsTreasuryVaultAuthority = async (
  globalConfig: Address
) => {
  const addressEncoder = getAddressEncoder();

  const [pda, bump] = await getProgramDerivedAddress({
    programAddress: KAMINO_FARMS_PROGRAM_ID,
    seeds: ["authority", addressEncoder.encode(globalConfig)],
  });

  return { address: pda, bump };
};

