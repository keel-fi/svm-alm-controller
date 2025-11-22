use crate::{constants::anchor_discriminator, cpi_instruction, integrations::cctp_bridge::constants::CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID};

cpi_instruction! {
    /// Deposit tokens for burn via CCTP (Cross-Chain Transfer Protocol).
    /// This instruction burns tokens on Solana and initiates a cross-chain transfer.
    pub struct DepositForBurn<'info> {
        program: CCTP_TOKEN_MESSENGER_MINTER_PROGRAM_ID,
        discriminator: anchor_discriminator("global", "deposit_for_burn"),
        accounts: {
            /// Controller authority that signs the transaction
            controller_authority: Signer,
            /// Payer for event rent
            event_rent_payer: Writable<Signer>,
            /// Sender authority PDA
            sender_authority_pda: Readonly,
            /// Vault token account to burn from
            vault: Writable,
            /// CCTP message transmitter state account
            message_transmitter: Writable,
            /// CCTP token messenger state account
            token_messenger: Readonly,
            /// Remote token messenger account
            remote_token_messenger: Readonly,
            /// Token minter account
            token_minter: Readonly,
            /// Local token account
            local_token: Writable,
            /// Mint of the token to burn
            burn_token_mint: Writable,
            /// Message sent event data account
            message_sent_event_data: Writable<Signer>,
            /// CCTP message transmitter program
            message_transmitter_program: Readonly,
            /// CCTP token messenger minter program
            token_messenger_minter_program: Readonly,
            /// Token program (Token or Token-2022)
            token_program: Readonly,
            /// System program
            system_program: Readonly,
            /// Event authority account
            event_authority: Readonly,
            /// CCTP program (duplicated for IDL compatibility)
            cctp_program: Readonly
        },
        args: {
            amount: u64,
            destination_domain: u32,
            mint_recipient: pinocchio::pubkey::Pubkey
        }
    }
}
