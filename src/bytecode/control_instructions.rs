use crate::jvm_model::FrameReturn;

use super::*;

pub fn goto_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let offset_base = frame.program_counter as isize - 1;
    let offset = read_i16_from_bytecode(frame) as isize;
    frame.program_counter = (offset_base + offset) as usize;

    Ok(())
}

pub fn return_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    frame.should_return = FrameReturn::Returning;
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
        frame.should_return = FrameReturn::Returning;
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

pub fn double_return_instruction(context: JvmContext) -> JvmResult<()> {
    generic_return_instruction(context, expect_double)
}
