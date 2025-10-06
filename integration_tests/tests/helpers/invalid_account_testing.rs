use litesvm::LiteSVM;
use solana_sdk::{
    instruction::Instruction, pubkey::Pubkey, signer::Signer, transaction::Transaction,
};
use std::collections::HashMap;

/// Represents different types of invalid account scenarios
#[derive(Debug, Clone)]
pub enum InvalidAccountType {
    /// Invalid owner - account exists but has wrong owner
    InvalidOwner,
    /// Invalid program ID - account exists but has wrong program ID
    InvalidProgramId,
    /// Account doesn't exist
    AccountNotFound,
    /// Account has invalid data (e.g., wrong mint for token account)
    InvalidData,
    /// Account is not initialized
    Uninitialized,
}

/// Configuration for testing invalid accounts
#[derive(Debug, Clone)]
pub struct InvalidAccountTestConfig {
    pub account_index: usize,
    pub invalid_type: InvalidAccountType,
    pub expected_error: solana_sdk::instruction::InstructionError,
    pub description: String,
    /// Optional: If we need to create a specific invalid account
    pub custom_invalid_account: Option<solana_sdk::account::Account>,
}

/// Builder for creating invalid account tests
pub struct InvalidAccountTestBuilder<'a> {
    svm: LiteSVM,
    payer: Pubkey,
    signers: Vec<Box<&'a dyn Signer>>,
    valid_instruction: Instruction,
    test_configs: Vec<InvalidAccountTestConfig>,
    account_backups: HashMap<usize, solana_sdk::account::Account>,
    original_account_keys: HashMap<usize, Pubkey>,
}

impl<'a> InvalidAccountTestBuilder<'a> {
    pub fn new(
        svm: LiteSVM,
        payer: Pubkey,
        signers: Vec<Box<&'a dyn Signer>>,
        valid_instruction: Instruction,
    ) -> Self {
        Self {
            svm,
            payer,
            signers,
            valid_instruction,
            test_configs: Vec::new(),
            account_backups: HashMap::new(),
            original_account_keys: HashMap::new(),
        }
    }

    /// Add a test case for invalid account owner
    pub fn with_invalid_owner(
        mut self,
        account_index: usize,
        expected_error: solana_sdk::instruction::InstructionError,
        description: &str,
    ) -> Self {
        self.test_configs.push(InvalidAccountTestConfig {
            account_index,
            invalid_type: InvalidAccountType::InvalidOwner,
            expected_error,
            description: description.to_string(),
            custom_invalid_account: None,
        });
        self
    }

    /// Add a test case for invalid program ID
    pub fn with_invalid_program_id(
        mut self,
        account_index: usize,
        expected_error: solana_sdk::instruction::InstructionError,
        description: &str,
    ) -> Self {
        self.test_configs.push(InvalidAccountTestConfig {
            account_index,
            invalid_type: InvalidAccountType::InvalidProgramId,
            expected_error,
            description: description.to_string(),
            custom_invalid_account: None,
        });
        self
    }

    /// Add a test case for account not found
    pub fn with_account_not_found(
        mut self,
        account_index: usize,
        expected_error: solana_sdk::instruction::InstructionError,
        description: &str,
    ) -> Self {
        self.test_configs.push(InvalidAccountTestConfig {
            account_index,
            invalid_type: InvalidAccountType::AccountNotFound,
            expected_error,
            description: description.to_string(),
            custom_invalid_account: None,
        });
        self
    }

    /// Add a test case for invalid data
    pub fn with_invalid_data(
        mut self,
        account_index: usize,
        expected_error: solana_sdk::instruction::InstructionError,
        description: &str,
    ) -> Self {
        self.test_configs.push(InvalidAccountTestConfig {
            account_index,
            invalid_type: InvalidAccountType::InvalidData,
            expected_error,
            description: description.to_string(),
            custom_invalid_account: None,
        });
        self
    }

    /// Add a test case with custom invalid account
    pub fn with_custom_invalid_account(
        mut self,
        account_index: usize,
        invalid_account: solana_sdk::account::Account,
        expected_error: solana_sdk::instruction::InstructionError,
        description: &str,
    ) -> Self {
        self.test_configs.push(InvalidAccountTestConfig {
            account_index,
            invalid_type: InvalidAccountType::InvalidData,
            expected_error,
            description: description.to_string(),
            custom_invalid_account: Some(invalid_account),
        });
        self
    }

    /// Run all the configured invalid account tests
    pub fn run_tests(mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Backup all accounts that will be modified and track original account keys
        for config in &self.test_configs {
            let account_pubkey = self.valid_instruction.accounts[config.account_index].pubkey;
            
            // Always backup the original account key for cases that modify the instruction
            if matches!(
                config.invalid_type,
                InvalidAccountType::InvalidProgramId | InvalidAccountType::AccountNotFound | InvalidAccountType::InvalidData
            ) {
                self.original_account_keys.insert(config.account_index, account_pubkey);
            }
            
            // Backup the account data if the account exists
            if let Some(account) = self.svm.get_account(&account_pubkey) {
                self.account_backups.insert(config.account_index, account);
            }
        }

        // Run each test
        let test_configs = self.test_configs.clone();
        for (test_idx, config) in test_configs.iter().enumerate() {
            println!("Running test {}: {}", test_idx + 1, config.description);

            // Apply the invalid account modification
            self.apply_invalid_account(config)?;

            // Execute the transaction
            let tx_result = self.execute_transaction();

            // Assert the expected error
            self.assert_error(&tx_result, config)?;

            // Restore the account
            self.restore_account(config)?;

            // Expire blockhash for next test
            self.svm.expire_blockhash();
        }

        Ok(())
    }

    fn apply_invalid_account(
        &mut self,
        config: &InvalidAccountTestConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match config.invalid_type {
            InvalidAccountType::InvalidOwner => {
                // Use the original account key for InvalidOwner cases
                let account_pubkey = self.original_account_keys
                    .get(&config.account_index)
                    .copied()
                    .unwrap_or(self.valid_instruction.accounts[config.account_index].pubkey);
                
                if let Some(account) = self.svm.get_account(&account_pubkey) {
                    let mut invalid_account = account.clone();
                    invalid_account.owner = Pubkey::new_unique();
                    self.svm.set_account(account_pubkey, invalid_account)?;
                }
            }
            InvalidAccountType::InvalidProgramId => {
                // For program ID validation, we change the account key to a random one
                // and create an account with the wrong program ID at that address
                let new_account_key = Pubkey::new_unique();
                self.valid_instruction.accounts[config.account_index].pubkey = new_account_key;
                
                // Create an account with the wrong program ID
                // For mint accounts, we need to create a proper mint account with wrong data
                // to get InvalidAccountData instead of InvalidAccountOwner
                let invalid_account = solana_sdk::account::Account {
                    lamports: 1_000_000_000, // Give it some lamports
                    data: vec![0; 82], // Proper mint account size but wrong data
                    owner: spl_token::ID, // Correct owner (SPL Token program)
                    executable: false,
                    rent_epoch: 0,
                };
                self.svm.set_account(new_account_key, invalid_account)?;
            }
            InvalidAccountType::AccountNotFound => {
                // Remove the account entirely
                self.valid_instruction.accounts[config.account_index].pubkey = Pubkey::new_unique();
            }
            InvalidAccountType::InvalidData => {
                if let Some(ref invalid_account) = config.custom_invalid_account {
                    // Use the original account key for custom invalid data
                    let account_pubkey = self.original_account_keys
                        .get(&config.account_index)
                        .copied()
                        .unwrap_or(self.valid_instruction.accounts[config.account_index].pubkey);
                    self.svm
                        .set_account(account_pubkey, invalid_account.clone())?;
                } else {
                    // Default: change to a random account
                    self.valid_instruction.accounts[config.account_index].pubkey =
                        Pubkey::new_unique();
                }
            }
            InvalidAccountType::Uninitialized => {
                // Use the original account key for uninitialized cases
                let account_pubkey = self.original_account_keys
                    .get(&config.account_index)
                    .copied()
                    .unwrap_or(self.valid_instruction.accounts[config.account_index].pubkey);
                
                // Create an uninitialized account
                let uninitialized_account = solana_sdk::account::Account {
                    lamports: 0,
                    data: vec![],
                    owner: Pubkey::new_unique(),
                    executable: false,
                    rent_epoch: 0,
                };
                self.svm
                    .set_account(account_pubkey, uninitialized_account)?;
            }
        }

        Ok(())
    }

    fn execute_transaction(&mut self) -> litesvm::types::TransactionResult {
        // Use a fresh blockhash for each transaction
        let blockhash = self.svm.latest_blockhash();
        
        // Convert Vec<Box<&dyn Signer>> to &[&dyn Signer]
        let signers_refs: Vec<&dyn solana_sdk::signer::Signer> = self.signers.iter().map(|s| s.as_ref() as &dyn solana_sdk::signer::Signer).collect();
        
        self.svm
            .send_transaction(Transaction::new_signed_with_payer(
                &[self.valid_instruction.clone()],
                Some(&self.payer),
                &signers_refs,
                blockhash,
            ))
    }

    fn assert_error(
        &self,
        tx_result: &litesvm::types::TransactionResult,
        config: &InvalidAccountTestConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let expected_error = &config.expected_error;

        assert!(
            matches!(
                tx_result,
                Err(litesvm::types::FailedTransactionMetadata {
                    err: solana_sdk::transaction::TransactionError::InstructionError(idx, err),
                    ..
                }) if *idx == 0 && err == expected_error
            ),
            "Test '{}' failed: Expected {:?}, but got {:?}",
            config.description,
            expected_error,
            tx_result.clone().err().unwrap().err
        );

        Ok(())
    }

    fn restore_account(
        &mut self,
        config: &InvalidAccountTestConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Restore the original account key if we modified the instruction
        if let Some(original_pubkey) = self.original_account_keys.get(&config.account_index) {
            self.valid_instruction.accounts[config.account_index].pubkey = *original_pubkey;
        }

        // Restore the original account data if we backed it up
        if let Some(backup_account) = self.account_backups.get(&config.account_index) {
            let account_pubkey = self.valid_instruction.accounts[config.account_index].pubkey;
            self.svm
                .set_account(account_pubkey, backup_account.clone())?;
        }

        Ok(())
    }
}

/// Convenience macro for creating invalid account tests
#[macro_export]
macro_rules! test_invalid_accounts {
    (
        $svm:expr,
        $payer:expr,
        $signers:expr,
        $instruction:expr,
        {
            $(
                $account_index:literal => $invalid_type:ident($expected_error:expr, $description:expr)
            ),*
            $(,)?
        }
    ) => {{
        use crate::helpers::invalid_account_testing::InvalidAccountTestBuilder;
        let mut builder = InvalidAccountTestBuilder::new($svm, $payer, $signers, $instruction);

        $(
            builder = match stringify!($invalid_type) {
                "invalid_owner" => builder.with_invalid_owner($account_index, $expected_error, $description),
                "invalid_program_id" => builder.with_invalid_program_id($account_index, $expected_error, $description),
                "account_not_found" => builder.with_account_not_found($account_index, $expected_error, $description),
                "invalid_data" => builder.with_invalid_data($account_index, $expected_error, $description),
                _ => panic!("Unknown invalid account type: {}", stringify!($invalid_type)),
            };
        )*

        builder.run_tests()
    }};
}
