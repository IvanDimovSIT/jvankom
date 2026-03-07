use super::*;

pub fn pop_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.top_frame();
    let poped_value = frame.operand_stack.pop();
    debug_assert!(poped_value.is_some());
    debug_assert!(!matches!(poped_value.unwrap(), JvmValue::Unusable));

    Ok(())
}

pub fn dup_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.top_frame();
    let peeked_value = frame.operand_stack.last().copied();
    if let Some(value) = peeked_value {
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

pub fn dup_x1_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.top_frame();
    let peeked_value = frame.operand_stack.last().copied();
    if let Some(value) = peeked_value {
        debug_assert!(!matches!(
            value,
            JvmValue::Long(_) | JvmValue::Double(_) | JvmValue::Unusable
        ));

        let insert_index = frame.operand_stack.len() - 2;
        frame.operand_stack.insert(insert_index, value);
    } else {
        return Err(JvmError::NoOperandFound.bx());
    }

    Ok(())
}
