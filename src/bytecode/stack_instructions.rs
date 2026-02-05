use super::*;

pub fn pop_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let poped_value = frame.operand_stack.pop();
    debug_assert!(poped_value.is_some());
    debug_assert!(!matches!(poped_value.unwrap(), JvmValue::Unusable));

    Ok(())
}
