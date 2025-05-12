use crate::helpers::constants::NOVA_TOKEN_SWAP_FEE_OWNER;
use crate::subs::{
    spl_token::{initialize_ata, initialize_mint},
    transfer_tokens,
};
use borsh::BorshSerialize;
use litesvm::LiteSVM;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::Transaction,
};
use std::error::Error;

const SWAP_LEN: usize = 414;

#[derive(Clone, Debug, Default, PartialEq, BorshSerialize)]
pub struct Fees {
    pub trade_fee_numerator: u64,
    pub trade_fee_denominator: u64,
    pub owner_trade_fee_numerator: u64,
    pub owner_trade_fee_denominator: u64,
    pub owner_withdraw_fee_numerator: u64,
    pub owner_withdraw_fee_denominator: u64,
    pub host_fee_numerator: u64,
    pub host_fee_denominator: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, BorshSerialize)]
pub enum CurveType {
    ConstantProduct,
    ConstantPrice,
    Offset,
    PythConstantPrice,
}

#[derive(Clone, Copy, Debug, PartialEq, BorshSerialize)]
pub struct ConstantProductCurve {
    pub curve_type: CurveType,
    pub calculator: [u8; 122],
}

#[derive(Clone, Debug, PartialEq, BorshSerialize)]
pub struct Initialize {
    pub fees: Fees,
    pub swap_curve: ConstantProductCurve,
}
impl Initialize {
    pub const LEN: usize = 188;
    pub const DISCRIMINATOR: u8 = 0;
    pub fn to_vec(&self) -> Vec<u8> {
        let mut serialized = Vec::with_capacity(1 + Self::LEN);
        serialized.push(Self::DISCRIMINATOR);
        BorshSerialize::serialize(self, &mut serialized).unwrap();
        serialized
    }
}

pub fn derive_swap_authority_pda_and_bump(swap: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
    let (pda, bump) = Pubkey::find_program_address(&[&swap.to_bytes()], &program_id);
    (pda, bump)
}

pub fn initialize_swap(
    svm: &mut LiteSVM,
    payer: &Keypair,
    authority: &Keypair,
    mint_a: &Pubkey,
    mint_b: &Pubkey,
    program_id: &Pubkey,
    initial_liquidity_a: u64,
    initial_liquidity_b: u64,
) -> Result<(Pubkey, Pubkey), Box<dyn Error>> {
    let swap_kp = Keypair::new();
    let swap_pk = swap_kp.pubkey();
    let (swap_authority, swap_authority_bump) =
        derive_swap_authority_pda_and_bump(&swap_pk, program_id);

    // Create the LP Mint
    let lp_mint_kp = Keypair::new();
    let lp_mint_pk = initialize_mint(svm, payer, &swap_authority, None, 2, Some(lp_mint_kp))?;

    // Create the LP ATA for the pool creator
    let creator_lp_mint_ata = initialize_ata(svm, payer, &authority.pubkey(), &lp_mint_pk)?;

    let fee_lp_mint_ata = initialize_ata(svm, payer, &NOVA_TOKEN_SWAP_FEE_OWNER, &lp_mint_pk)?;

    // Create the swap vault for mint a and b
    let swap_token_a = initialize_ata(svm, payer, &swap_authority, &mint_a)?;
    let swap_token_b = initialize_ata(svm, payer, &swap_authority, &mint_b)?;

    // Transfer the initial liquidity
    transfer_tokens(
        svm,
        payer,
        authority,
        mint_a,
        &swap_authority,
        initial_liquidity_a,
    )?;
    transfer_tokens(
        svm,
        payer,
        authority,
        mint_b,
        &swap_authority,
        initial_liquidity_b,
    )?;

    let args = Initialize {
        fees: Fees {
            trade_fee_numerator: 0,
            trade_fee_denominator: 0,
            owner_trade_fee_numerator: 0,
            owner_trade_fee_denominator: 0,
            owner_withdraw_fee_numerator: 0,
            owner_withdraw_fee_denominator: 0,
            host_fee_numerator: 0,
            host_fee_denominator: 0,
        },
        swap_curve: ConstantProductCurve {
            curve_type: CurveType::ConstantProduct,
            calculator: [0; 122],
        },
    };

    let create_account_ixn = solana_system_interface::instruction::create_account(
        &payer.pubkey(),
        &swap_pk,
        svm.minimum_balance_for_rent_exemption(SWAP_LEN),
        SWAP_LEN as u64,
        program_id,
    );

    let init_swap_pool_ixn = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta {
                pubkey: swap_pk,
                is_signer: true,
                is_writable: true,
            },
            AccountMeta {
                pubkey: swap_authority,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: swap_token_a,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: swap_token_b,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: lp_mint_pk,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: fee_lp_mint_ata,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: creator_lp_mint_ata,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: pinocchio_token::ID.into(),
                is_signer: false,
                is_writable: false,
            },
        ],
        data: args.to_vec(),
    };

    //   0. `[writable, signer]` New Token-swap to create.
    //   1. `[]` swap authority derived from
    //      `create_program_address(&[Token-swap account])`
    //   2. `[]` token_a Account. Must be non zero, owned by swap authority.
    //   3. `[]` token_b Account. Must be non zero, owned by swap authority.
    //   4. `[writable]` Pool Token Mint. Must be empty, owned by swap
    //      authority.
    //   5. `[]` Pool Token Account to deposit trading and withdraw fees. Must
    //      be empty, not owned by swap authority
    //   6. `[writable]` Pool Token Account to deposit the initial pool token
    //      supply. Must be empty, not owned by swap authority.
    //   7. `[]` Pool Token program id

    let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
        &[create_account_ixn, init_swap_pool_ixn],
        Some(&payer.pubkey()),
        &[&payer, &swap_kp],
        svm.latest_blockhash(),
    ));
    if tx_result.is_err() {
        println!("{:#?}", tx_result.unwrap().logs);
    } else {
        assert!(tx_result.is_ok(), "Transaction failed to execute");
    }

    let swap_acc = svm.get_account(&swap_pk);
    println!("{:#?}", swap_acc.unwrap());

    // let mint_acc = svm.get_account(&mint_kp.pubkey());
    // let mint_data = mint_acc.unwrap().data;
    // let mint = Mint::unpack(&mint_data).map_err(|e| format!("Failed to unpack mint: {:?}", e))?;

    // assert_eq!(mint.decimals, 6, "Incorrect number of decimals");
    // assert_eq!(mint.mint_authority, COption::Some(*mint_authority), "Incorrect mint_authority");
    // assert_eq!(mint.freeze_authority, freeze_authority.map(|fa| COption::Some(*fa)).unwrap_or(COption::None), "Incorrect freeze_authority");

    Ok((swap_pk, lp_mint_pk))
}
