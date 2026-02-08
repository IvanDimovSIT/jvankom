use super::*;

pub fn return_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    frame.should_return = true;

    Ok(())
}

pub fn integer_return_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    if let Some(value_to_return) = frame.operand_stack.pop() {
        expect_int(value_to_return)?;
        frame.should_return = true;
        frame.return_value = Some(value_to_return);
    } else {
        return Err(JvmError::NoOperandFound.bx());
    }

    Ok(())
}

pub fn object_return_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    if let Some(value_to_return) = frame.operand_stack.pop() {
        expect_reference(value_to_return)?;
        frame.should_return = true;
        frame.return_value = Some(value_to_return);
    } else {
        return Err(JvmError::NoOperandFound.bx());
    }

    Ok(())
}
