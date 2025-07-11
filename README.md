# Nova SVM ALM Controller

The Nova SVM ALM (Asset-Liability Management) Controller is intended to facilitate the controlled management of asset and liability positions on behalf of the Nova STAR on Solana or other SVM chains.

In effect, it is a glorified program-owned wallet with:
- role permissioning for external wallets
- rate limiting
- configurable integrations with DeFi protocols
- integrations with cross-chain bridges
- audit-trail generation with all actions

## Core Concepts

### Controller

The account with a PDA that acts as the signer and owner on all key balances, positions, etc. Multiple Controller instances can exist in a single deployment of the program. The Controller account state itself is rather limited, with the exception of a high-level status which can be used to suspend all actions by the Controller in extreme cases.

A System Program owned PDA with no data, the "controller_authority", is used for all signatures to ensure safety with signing CPIs.

### Permissions

Permission accounts are unique by Controller and Authority (an external wallet). The external wallet could be a governance-instructed multi-sig (for example, where configuration permissions are granted), or "relayer" wallets delegated with only limited positions (for example, rebalancing actions).

These accounts contain rules determining the types of actions that a given caller can invoke.

## Reserves 

Reserve accounts are used to track the balances and flows of a particular SPL Token. They are 1-to-1 with an Associated Token Account owned by the Controller authority PDA. These accounts contain state which can be used to track changes to the SPL Token balance (for example, generating accounting events to account for permissionless inflows into the Controller's ATA). Rate limiting can also be applied at Reserve level (in addition to at Intergration level).

Although anyone could permissionessly transfer funds to the Controller, actions cannot be taken in respect of a Controller authority's ATA unless there is a Reserve configured for it. 

## Integrations 

Integrations are intended to act as the basis for a broad range of different protocols and types of interfaces that the Controller may need to interface with. A non-exhaustive list of Integratons is set out below to provide some examples.

Flexibility comes from the use of enum structs in respect of the `IntegrationConfig` and `IntegrationState` values which differ based on the program's which are to be interfaced with.

Support for different types of Integration will require modules to be developed to interface with each various other program(s). 

There are "outer" handlers for `Initialize`, `Sync`, `Push` and `Pull` actions. Each Integration will have it's own modules which will need to implement the "inner" handler logic (as well as defining "inner" account contexts and args) for any actions which are applicable to it.

It's critical that all token outflows from `Push` actions or inflows from `Pull` actions are correctly accounted for within their respective Reserve AND Integration accounts.

### Core Integrations

| Integration | Initialize | Sync | Push | Pull | Other        |
|-------------|------------|------|------|------|--------------|
| SplTokenExternal | Yes | No | Yes | No | No |
| SplTokenSwap | Yes | Yes | Yes | Yes | No |
| CctpBridge | Yes | No | Yes | No | No |
| LzBridge | Yes | No | Yes | No | No |
| AtomicSwap | Yes | Yes | Yes | No | Borrow, Repay |

#### SplTokenExternal
Enables the transferring of tokens from a Controller owned TokenAccount to an external wallet. The implementation only supports the transferring to a recipients Associated Token Account (ATA). The ATA will be created if the recipient does not have an initialized ATA.

### Future Integrations

Future integrations are likely to include interfaces with DeFi protocols across Solana, for example:
- Kamino Lend
- Drift Spot (borrow-lend) markets
- Save (fka Solend)
- MarginFi



## Build

From project root

```
cargo build-sbf
```

## Generating IDL

This repository uses Shank for IDL generation.

Install the Shank CLI

```
cargo install shank-cli
```

Generate IDL

```
shank idl -r program -o idl
// OR
yarn generate-idl
```

## Generating Clients

_Ensure the IDL has been created or updated using the above IDL generation steps._

Install dependencies

```
yarn install
```

Run client generation script

```
yarn generate-clients
```

## Running Tests

Integration tests are written using [LiteSvm](https://github.com/LiteSVM/litesvm). To run integration tests, from project root build and then run

```
cargo test
```

### If running into issues with openssl
Add environment variables manually to point to a particular version of openssl@3
```
OPENSSL_DIR="/usr/local/opt/openssl@3" OPENSSL_INCLUDE_DIR="/usr/local/opt/openssl@3/include" OPENSSL_LIB_DIR="/usr/local/opt/openssl@3/lib" OPENSSL_NO_VENDOR="1" cargo test
```

Apple silicon:
```
OPENSSL_DIR="/opt/homebrew/opt/openssl@3" OPENSSL_INCLUDE_DIR="/opt/homebrew/opt/openssl@3/include" OPENSSL_LIB_DIR="/opt/homebrew/opt/openssl@3/lib" OPENSSL_NO_VENDOR="1" cargo test
```