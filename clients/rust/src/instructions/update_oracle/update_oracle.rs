use crate::{
    derive_controller_authority_pda, generated::{instructions::UpdateOracleBuilder, types::FeedArgs},
};
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;


pub fn create_update_oracle_instruction(
    controller: &Pubkey,
    authority: &Pubkey,
    oracle: &Pubkey,
    price_feed: &Pubkey,
    feed_args: Option<FeedArgs>,
    new_authority: Option<&Pubkey>,
) -> Instruction {
    let controller_authority = derive_controller_authority_pda(controller);
    let new_authority_pubkey = new_authority.map(|k| *k);
    let ixn = if let Some(feed_args) = feed_args {
        UpdateOracleBuilder::new()
            .controller(*controller)
            .controller_authority(controller_authority)
            .authority(*authority)
            .oracle(*oracle)
            .price_feed(*price_feed)
            .feed_args(feed_args)
            .new_authority(new_authority_pubkey)
            .instruction()
    } else {
        UpdateOracleBuilder::new()
            .controller(*controller)
            .controller_authority(controller_authority)
            .authority(*authority)
            .oracle(*oracle)
            .price_feed(*price_feed)
            .new_authority(new_authority_pubkey)
            .instruction()
    };
    ixn
}
