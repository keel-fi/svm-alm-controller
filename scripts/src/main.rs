use anyhow::Result;
use clap::Parser;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::AccountMeta, message::Message, pubkey::Pubkey, signature::Keypair, signer::Signer, system_program, transaction::Transaction
};
use std::{fs, str::FromStr};
use svm_alm_controller_client::{generated::{
    accounts::{Controller, Permission},
    instructions::InitializeControllerBuilder,
    types::ControllerStatus,
}, SVM_ALM_CONTROLLER_ID};
use borsh::BorshDeserialize;
use base64::{engine::general_purpose, Engine as _};
use bincode;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// RPC URL for the Solana network
    #[arg(long)]
    rpc_url: String,

    /// Path to payer keypair JSON file
    #[arg(long)]
    payer_keypair_path: String,

    /// Path to authority keypair JSON file
    #[arg(long)]
    authority_keypair_path: String,

    /// Controller status (0 = Active, 1 = Suspended)
    #[arg(long, default_value = "0")]
    status: u8,

    /// Controller ID
    #[arg(long)]
    id: u16,
}

fn derive_controller_pda(id: &u16, program_id: &Pubkey) -> Pubkey {
    let (controller_pda, _controller_bump) = Pubkey::find_program_address(
        &[b"controller", &id.to_le_bytes()],
        program_id,
    );
    controller_pda
}

fn derive_controller_authority_pda(controller_pda: &Pubkey, program_id: &Pubkey) -> Pubkey {
    let (controller_authority_pda, _controller_authority_bump) = Pubkey::find_program_address(
        &[b"controller_authority", controller_pda.as_ref()],
        program_id
    );
    controller_authority_pda
}

fn derive_permission_pda(controller_pda: &Pubkey, authority: &Pubkey, program_id: &Pubkey) -> Pubkey {
    let (permission_pda, _permission_bump) = Pubkey::find_program_address(
        &[b"permission", controller_pda.as_ref(), authority.as_ref()],
        program_id,
    );
    permission_pda
}

fn load_keypair_from_file(path: &str) -> Result<Keypair> {
    let keypair_data = fs::read_to_string(path)?;
    let keypair_bytes: Vec<u8> = serde_json::from_str(&keypair_data)?;
    Keypair::from_bytes(&keypair_bytes).map_err(Into::into)
}

fn fetch_controller_account(
    client: &RpcClient,
    controller_pda: &Pubkey,
) -> Result<Option<Controller>> {
    let controller_info = client.get_account(controller_pda)?;
    if controller_info.data.is_empty() {
        Ok(None)
    } else {
        Controller::from_bytes(&controller_info.data[1..])
            .map(Some)
            .map_err(Into::into)
    }
}

fn fetch_permission_account(
    client: &RpcClient,
    permission_pda: &Pubkey,
) -> Result<Option<Permission>> {
    let permission_info = client.get_account(permission_pda)?;
    if permission_info.data.is_empty() {
        Ok(None)
    } else {
        Permission::from_bytes(&permission_info.data[1..])
            .map(Some)
            .map_err(Into::into)
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Load keypairs from files
    let payer = load_keypair_from_file(&args.payer_keypair_path)?;
    let authority = load_keypair_from_file(&args.authority_keypair_path)?;

    // Parse program ID
    // let program_id = Pubkey::from_str(&args.program_id)?;
    let program_id = SVM_ALM_CONTROLLER_ID;

    // Parse status
    let status = match args.status {
        0 => ControllerStatus::Active,
        1 => ControllerStatus::Suspended,
        _ => return Err(anyhow::anyhow!("Invalid status value. Must be 0 (Active) or 1 (Suspended)")),
    };

    // Connect to RPC
    let client = RpcClient::new(args.rpc_url);

    // Debug: Check if program exists
    println!("Checking if program exists: {}", program_id);
    match client.get_account(&program_id) {
        Ok(account) => {
            println!("✅ Program found! Owner: {}", account.owner);
            println!("   Lamports: {}", account.lamports);
            println!("   Data length: {}", account.data.len());
        }
        Err(e) => {
            eprintln!("❌ Program not found: {:#?}", e);
            return Err(anyhow::anyhow!("Program {} not found on network", program_id));
        }
    }

    // Derive PDAs
    let controller_pda = derive_controller_pda(&args.id, &program_id);
    let controller_authority_pda = derive_controller_authority_pda(&controller_pda, &program_id);
    let permission_pda = derive_permission_pda(&controller_pda, &authority.pubkey(), &program_id);

    println!("Deploying controller with ID: {}", args.id);
    println!("Controller PDA: {}", controller_pda);
    println!("Permission PDA: {}", permission_pda);
    println!("Status: {:?}", status);

    // Check if controller already exists
    // let existing_controller = fetch_controller_account(&client, &controller_pda)?;
    // if existing_controller.is_some() {
    //     return Err(anyhow::anyhow!("Controller with ID {} already exists", args.id));
    // }

    // Create instruction
    let ixn = InitializeControllerBuilder::new()
        .id(args.id)
        .status(status)
        .payer(payer.pubkey())
        .authority(authority.pubkey())
        .controller(controller_pda)
        .controller_authority(controller_authority_pda)
        .permission(permission_pda)
        .system_program(system_program::ID)
        .program_id(program_id)
        .add_remaining_account(AccountMeta{ pubkey: program_id, is_signer: false, is_writable: false })
        .instruction();

    // Debug: print instruction data
    println!("Instruction: {:?}", ixn);
    println!("Instruction data (base64): {}", general_purpose::STANDARD.encode(&ixn.data));
    println!("Instruction program_id: {}", ixn.program_id);
    println!("Instruction accounts: {:?}", ixn.accounts);

    // Get recent blockhash
    let recent_blockhash = client.get_latest_blockhash()?;

    // Create and sign transaction
    let message = Message::new(&[ixn.clone()], Some(&payer.pubkey()));
    println!("Transaction message: {:?}", message);
    println!("Signers: payer={}, authority={}", payer.pubkey(), authority.pubkey());

    let txn = Transaction::new_signed_with_payer(
        &[ixn],
        Some(&payer.pubkey()),
        &[&authority, &payer],
        recent_blockhash,
    );

    // Debug: print transaction (base64)
    let serialized_tx = bincode::serialize(&txn)?;
    println!("Serialized transaction (base64): {}", general_purpose::STANDARD.encode(&serialized_tx));

    // Send transaction
    println!("Sending transaction...");
    match client.send_and_confirm_transaction(&txn) {
        Ok(signature) => {
            println!("Transaction successful! Signature: {}", signature);
        }
        Err(e) => {
            eprintln!("Transaction failed: {:#?}", e);
            return Err(e.into());
        }
    }

    // Verify deployment
    println!("Verifying deployment...");
    let controller = fetch_controller_account(&client, &controller_pda)?;
    let permission = fetch_permission_account(&client, &permission_pda)?;

    if let Some(controller) = controller {
        println!("✅ Controller deployed successfully!");
        println!("  - ID: {}", controller.id);
        println!("  - Bump: {}", controller.bump);
        println!("  - Status: {:?}", controller.status);
    } else {
        return Err(anyhow::anyhow!("Controller account not found after deployment"));
    }

    if let Some(permission) = permission {
        println!("✅ Permission account created successfully!");
        println!("  - Authority: {}", permission.authority);
        println!("  - Controller: {}", permission.controller);
        println!("  - Status: {:?}", permission.status);
    } else {
        return Err(anyhow::anyhow!("Permission account not found after deployment"));
    }

    Ok(())
} 