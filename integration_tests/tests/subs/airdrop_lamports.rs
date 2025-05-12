use litesvm::LiteSVM;
use solana_sdk::pubkey::Pubkey;
use std::error::Error;

pub fn airdrop_lamports(
    svm: &mut LiteSVM,
    recipient: &Pubkey,
    amount: u64,
) -> Result<(), Box<dyn Error>> {
    let recipient_balance_before = svm
        .get_account(recipient)
        .map_or(0, |account| account.lamports);

    svm.airdrop(recipient, amount)
        .map_err(|e| format!("Airdrop failed: {:?}", e))?;

    let recipient_balance_after = svm
        .get_account(recipient)
        .map_or(0, |account| account.lamports);

    let delta = recipient_balance_after
        .checked_sub(recipient_balance_before)
        .ok_or("Overflow error")?;

    assert_eq!(
        delta, amount,
        "The recipient's balance did not increase by the expected amount after airdrop."
    );

    Ok(())
}
