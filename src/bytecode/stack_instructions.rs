use super::*;

pub fn pop_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let poped_value = frame.operand_stack.pop();
    debug_assert!(poped_value.is_some());
    debug_assert!(!matches!(poped_value.unwrap(), JvmValue::Unusable));

    Ok(())
}

pub fn dup_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let poped_value = frame.operand_stack.last().copied();
    if let Some(value) = poped_value {
        debug_assert!(!matches!(
            value,
            JvmValue::Long(_) | JvmValue::Double(_) | JvmValue::Unusable
        ));

        frame.operand_stack.push(value);
    } else {
        return Err(JvmError::NoOperandFound.bx());
    }

    Ok(())
}
