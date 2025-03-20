use borsh::{maybestd::vec::Vec, BorshSerialize};
use pinocchio::{account_info::AccountInfo, instruction::{AccountMeta, Instruction, Signer}, program::invoke_signed, program_error::ProgramError, pubkey::Pubkey};


#[derive(BorshSerialize, Debug, PartialEq, Eq, Clone,)]
pub struct DepositForBurnArgs {
    pub amount: u64,
    pub destination_domain: u32,
    pub mint_recipient: Pubkey,
}

impl DepositForBurnArgs {

    pub const DISCRIMINATOR: [u8;8] = [0,0,0,0,0,0,0,0];
    pub const LEN: usize = 44;

    pub fn to_vec(&self) -> Result<Vec<u8>, ProgramError> {
        let mut serialized: Vec<u8> = Vec::with_capacity(8 + Self::LEN);
        serialized.extend_from_slice(&Self::DISCRIMINATOR);
        BorshSerialize::serialize(self, &mut serialized).unwrap();
        Ok(serialized)
    } 
    
}


pub fn deposit_for_burn_cpi(
    amount: u64,
    destination_domain: u32,
    mint_recipient: Pubkey,
    signer: Signer,
    cctp_program: Pubkey,
    controller: &AccountInfo,
    event_rent_payer: &AccountInfo,
    sender_authority_pda: &AccountInfo,
    vault: &AccountInfo,
    message_transmitter: &AccountInfo,
    token_messenger: &AccountInfo,
    remote_token_messenger: &AccountInfo,
    token_minter: &AccountInfo,
    local_token: &AccountInfo,
    burn_token_mint: &AccountInfo,
    message_sent_event_data: &AccountInfo,
    message_transmitter_program: &AccountInfo,
    token_messenger_minter_program: &AccountInfo,
    token_program: &AccountInfo,
    system_program: &AccountInfo,
) -> Result<(), ProgramError> {
    let args_vec = DepositForBurnArgs {
        amount: amount,
        destination_domain: destination_domain,
        mint_recipient: mint_recipient,
    }.to_vec().unwrap();
    let data = args_vec.as_slice();
    invoke_signed(
        &Instruction {
            program_id: &cctp_program,
            data: &data,
            accounts: &[
                AccountMeta::readonly_signer(controller.key()),
                AccountMeta::writable_signer(event_rent_payer.key()),
                AccountMeta::readonly(sender_authority_pda.key()),
                AccountMeta::writable(vault.key()),
                AccountMeta::writable(message_transmitter.key()),
                AccountMeta::readonly(token_messenger.key()),
                AccountMeta::readonly(remote_token_messenger.key()),
                AccountMeta::readonly(token_minter.key()),
                AccountMeta::writable(local_token.key()),
                AccountMeta::writable(burn_token_mint.key()),
                AccountMeta::writable(message_sent_event_data.key()),
                AccountMeta::readonly(message_transmitter_program.key()),
                AccountMeta::readonly(token_messenger_minter_program.key()),
                AccountMeta::readonly(token_program.key()),
                AccountMeta::readonly(system_program.key()),
            ]
        },
        &[
            controller,                 // owner
            event_rent_payer,
            sender_authority_pda,
            vault,                      // burn_token_account,
            message_transmitter,
            token_messenger, 
            remote_token_messenger,
            token_minter,
            local_token,
            burn_token_mint,
            message_sent_event_data,
            message_transmitter_program,
            token_messenger_minter_program,
            token_program,
            system_program,
        ], 
        &[
            signer
        ]
    )?;
    Ok(())
}