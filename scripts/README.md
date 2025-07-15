# Controller Deployment Script

A simple Rust script to deploy SVM ALM Controller instances.

## Usage

```bash
cargo run -- \
  --rpc-url <RPC_URL> \
  --program-id <PROGRAM_ID> \
  --payer-keypair-path <PATH_TO_PAYER_KEYPAIR.json> \
  --authority-keypair-path <PATH_TO_AUTHORITY_KEYPAIR.json> \
  --id <CONTROLLER_ID> \
  --status <STATUS>
```

## Arguments

- `--rpc-url`: RPC URL for the Solana network (e.g., `https://api.mainnet-beta.solana.com`)
- `--program-id`: The SVM ALM Controller program ID
- `--payer-keypair-path`: Path to the payer keypair JSON file
- `--authority-keypair-path`: Path to the authority keypair JSON file
- `--id`: Controller ID (u16)
- `--status`: Controller status (0 = Active, 1 = Paused, 2 = Frozen, default: 0)

## Example

```bash
cargo run -- \
  --rpc-url https://api.devnet.solana.com \
  --program-id YourProgramIdHere \
  --payer-keypair-path ~/.config/solana/id.json \
  --authority-keypair-path ~/.config/solana/authority.json \
  --id 1 \
  --status 0
```

## Keypair Format

The script expects keypair files in the standard Solana JSON format (array of numbers representing the private key bytes).

You can generate a new keypair using:
```bash
solana-keygen new --outfile keypair.json
``` 