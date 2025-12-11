import {
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
    programAddress: SVM_ALM_CONTROLLER_PROGRAM_ADDRESS,
    seeds: ["integration", addressEncoder.encode(controller), integrationHash],
  });

  return integrationPda;
};

export const deriveControllerPda = async (id: number) => {
  const [controllerPda] = await getProgramDerivedAddress({
    programAddress: SVM_ALM_CONTROLLER_PROGRAM_ADDRESS,
    seeds: ["controller", Buffer.from(new Uint16Array([id]).buffer)],
  });

  return controllerPda;
};

export const deriveControllerAuthorityPda = async (controller: Address) => {
  const addressEncoder = getAddressEncoder();

  const [controllerAuthorityPda] = await getProgramDerivedAddress({
    programAddress: SVM_ALM_CONTROLLER_PROGRAM_ADDRESS,
    seeds: ["controller_authority", addressEncoder.encode(controller)],
  });

  return controllerAuthorityPda;
};

export const derivePermissionPda = async (
  controller: Address,
  authority: Address
) => {
  const addressEncoder = getAddressEncoder();

  const [permissionPda] = await getProgramDerivedAddress({
    programAddress: SVM_ALM_CONTROLLER_PROGRAM_ADDRESS,
    seeds: [
      "permission",
      addressEncoder.encode(controller),
      addressEncoder.encode(authority),
    ],
  });

  return permissionPda;
};

export const deriveReservePda = async (controller: Address, mint: Address) => {
  const addressEncoder = getAddressEncoder();

  const [reservePda] = await getProgramDerivedAddress({
    programAddress: SVM_ALM_CONTROLLER_PROGRAM_ADDRESS,
    seeds: [
      "reserve",
      addressEncoder.encode(controller),
      addressEncoder.encode(mint),
    ],
  });

  return reservePda;
};
