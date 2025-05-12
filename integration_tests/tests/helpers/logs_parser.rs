use litesvm::types::TransactionMetadata;
use solana_sdk::instruction::CompiledInstruction;

/// Prints all inner instructions in a transaction
pub fn print_inner_instructions(tx_metadata: &TransactionMetadata) {
    println!("Transaction inner instructions:");

    // Check if there are any inner instructions
    if tx_metadata.inner_instructions.is_empty() {
        println!("  No inner instructions found");
        return;
    }

    // Iterate through each inner instruction group
    for (idx, inner_instructions_group) in tx_metadata.inner_instructions.iter().enumerate() {
        println!(
            "  Instruction index {}: {} inner instructions",
            idx,
            inner_instructions_group.len()
        );

        // Iterate through each inner instruction in the group
        for (inner_idx, inner_instruction) in inner_instructions_group.iter().enumerate() {
            println!(
                "  Inner Ixn index {:?} ({:?}): {:?} : {:?} ",
                inner_idx,
                inner_instruction.stack_height,
                inner_instruction.instruction.accounts,
                inner_instruction.instruction.data,
            );
            // print_instruction(inner_idx, inner_instruction);
        }
    }
}

// /// Helper function to print a single instruction
// fn print_instruction(idx: usize, instruction: &CompiledInstruction) {
//     println!("    Inner instruction {}: ", idx);
//     println!("      Program index: {}", instruction.program_id_index);
//     println!("      Account indices: {:?}", instruction.accounts);
//     println!("      Data (hex): {}",&instruction.data);
// }
