use super::*;

pub fn return_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    frame.should_return = true;

    Ok(())
}

#[inline]
fn generic_return_instruction<F, T>(context: JvmContext, expect_generic: F) -> JvmResult<()>
where
    F: FnOnce(JvmValue) -> JvmResult<T>,
{
    let frame = context.current_thread.peek().unwrap();
    if let Some(value_to_return) = frame.operand_stack.pop() {
        expect_generic(value_to_return)?;
        frame.should_return = true;
        frame.return_value = Some(value_to_return);
    } else {
        return Err(JvmError::NoOperandFound.bx());
    }

    Ok(())
}

pub fn integer_return_instruction(context: JvmContext) -> JvmResult<()> {
    generic_return_instruction(context, expect_int)
}

pub fn object_return_instruction(context: JvmContext) -> JvmResult<()> {
    generic_return_instruction(context, expect_reference)
}
