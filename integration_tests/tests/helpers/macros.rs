/// Asserts that the TransactionMetadata contains a valid Controller
/// CPI event.
#[macro_export]
macro_rules! assert_contains_controller_cpi_event {
    ($tx_meta:expr, $account_keys:expr, $svm_controller_event:expr) => {
        let mut found = false;

        for (_ix_index, ix) in $tx_meta.inner_instructions.iter().enumerate() {
            for (_inner_ix_index, inner_instruction) in ix.iter().enumerate() {
                // Inner instruction is CPI event from our program
                if svm_alm_controller_client::SVM_ALM_CONTROLLER_ID
                    .eq(inner_instruction.instruction.program_id(&$account_keys))
                    && inner_instruction.instruction.data[0] == 0
                {
                    // 1 byte for IX disc
                    // 2 bytes for Controller ID
                    // 4 bytes for Vec length encoding
                    let event_data = &inner_instruction.instruction.data[7..];
                    let event_res = SvmAlmControllerEvent::try_from_slice(event_data);
                    match event_res {
                      Ok(event) => {
                        if event == $svm_controller_event {
                            found = true;
                            break;
                        }

                      },
                      Err(_e) => {
                        println!("Failed to deserialze Controller CPI event");
                      }
                    }
                }
            }
            if found {
                break;
            }
        }
        if !found {
            panic!("No matching Controller CPI event found");
        }
    };
}
