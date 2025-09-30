import {
  address,
  Address,
  getAddressEncoder,
  getProgramDerivedAddress,
} from "@solana/addresses";
import { ReadonlyUint8Array } from "@solana/kit";
import { SVM_ALM_CONTROLLER_PROGRAM_ADDRESS } from "./generated";

export const deriveIntegrationPda = async (
  controller: Address,
  integrationHash: ReadonlyUint8Array
) => {
  const addressEncoder = getAddressEncoder();

  // Derive the integration PDA using the integration hash
  const [integrationPda] = await getProgramDerivedAddress({
    programAddress: address(SVM_ALM_CONTROLLER_PROGRAM_ADDRESS),
    seeds: ["integration", addressEncoder.encode(controller), integrationHash],
  });

  return integrationPda;
};

export const deriveControllerPda = async (id: number) => {
  const [controllerPda] = await getProgramDerivedAddress({
    programAddress: address(SVM_ALM_CONTROLLER_PROGRAM_ADDRESS),
    seeds: ["controller", Buffer.from(new Uint16Array([id]).buffer)],
  });

  return controllerPda;
};

export const deriveControllerAuthorityPda = async (controller: Address) => {
  const addressEncoder = getAddressEncoder();

  const [controllerAuthorityPda] = await getProgramDerivedAddress({
    programAddress: address(SVM_ALM_CONTROLLER_PROGRAM_ADDRESS),
    seeds: ["controller_authority", addressEncoder.encode(address(controller))],
  });

  return controllerAuthorityPda;
};

export const derivePermissionPda = async (
  controller: Address,
  authority: Address
) => {
  const addressEncoder = getAddressEncoder();

  const [permissionPda] = await getProgramDerivedAddress({
    programAddress: address(SVM_ALM_CONTROLLER_PROGRAM_ADDRESS),
    seeds: [
      "permission",
      addressEncoder.encode(address(controller)),
      addressEncoder.encode(address(authority)),
    ],
  });

  return permissionPda;
};

export const deriveReservePda = async (controller: Address, mint: Address) => {
  const addressEncoder = getAddressEncoder();

  const [reservePda] = await getProgramDerivedAddress({
    programAddress: address(SVM_ALM_CONTROLLER_PROGRAM_ADDRESS),
    seeds: [
      "reserve",
      addressEncoder.encode(address(controller)),
      addressEncoder.encode(address(mint)),
    ],
  });

  return reservePda;
};

export const deriveSplTokenSwapLpTokenPda = async (
  controller: Address,
  lpMintAddress: Address
) => {
  const addressEncoder = getAddressEncoder();

  const [lpTokenPda] = await getProgramDerivedAddress({
    programAddress: address(SVM_ALM_CONTROLLER_PROGRAM_ADDRESS),
    seeds: [
      "spl-swap-lp",
      addressEncoder.encode(address(controller)),
      addressEncoder.encode(address(lpMintAddress)),
    ],
  });

  return lpTokenPda;
};
