use super::*;

pub fn if_equals_instruction(context: JvmContext) -> JvmResult<()> {
    generic_if_instruction(context, |int| int == 0)
}

pub fn if_not_equals_instruction(context: JvmContext) -> JvmResult<()> {
    generic_if_instruction(context, |int| int != 0)
}

pub fn if_less_than_instruction(context: JvmContext) -> JvmResult<()> {
    generic_if_instruction(context, |int| int < 0)
}

pub fn if_less_than_or_equals_instruction(context: JvmContext) -> JvmResult<()> {
    generic_if_instruction(context, |int| int <= 0)
}

pub fn if_greater_than_instruction(context: JvmContext) -> JvmResult<()> {
    generic_if_instruction(context, |int| int > 0)
}

pub fn if_greater_than_or_equals_instruction(context: JvmContext) -> JvmResult<()> {
    generic_if_instruction(context, |int| int >= 0)
}

pub fn if_compare_equals_instruction(context: JvmContext) -> JvmResult<()> {
    generic_if_compare_instruction(context, |a, b| a == b)
}

pub fn if_compare_not_equals_instruction(context: JvmContext) -> JvmResult<()> {
    generic_if_compare_instruction(context, |a, b| a != b)
}

pub fn if_compare_less_than_instruction(context: JvmContext) -> JvmResult<()> {
    generic_if_compare_instruction(context, |a, b| a < b)
}

pub fn if_compare_less_than_or_equals_instruction(context: JvmContext) -> JvmResult<()> {
    generic_if_compare_instruction(context, |a, b| a <= b)
}

pub fn if_compare_greater_than_instruction(context: JvmContext) -> JvmResult<()> {
    generic_if_compare_instruction(context, |a, b| a > b)
}

pub fn if_compare_greater_than_or_equals_instruction(context: JvmContext) -> JvmResult<()> {
    generic_if_compare_instruction(context, |a, b| a >= b)
}

#[inline]
fn generic_if_instruction<F>(context: JvmContext, logic_fn: F) -> JvmResult<()>
where
    F: FnOnce(i32) -> bool,
{
    let frame = context.current_thread.top_frame();
    let int = pop_int(frame)?;
    if !logic_fn(int) {
        // skip branch location
        frame.program_counter += 2;
        return Ok(());
    }

    let instruction_start = frame.program_counter as isize - 1;
    let branch_offset = read_i16_from_bytecode(frame) as isize;
    frame.program_counter = (instruction_start + branch_offset) as usize;

    Ok(())
}

#[inline]
fn generic_if_compare_instruction<F>(context: JvmContext, logic_fn: F) -> JvmResult<()>
where
    F: FnOnce(i32, i32) -> bool,
{
    let frame = context.current_thread.top_frame();
    let value2 = pop_int(frame)?;
    let value1 = pop_int(frame)?;
    if !logic_fn(value1, value2) {
        // skip branch location
        frame.program_counter += 2;
        return Ok(());
    }

    let instruction_start = frame.program_counter as isize - 1;
    let branch_offset = read_i16_from_bytecode(frame) as isize;
    frame.program_counter = (instruction_start + branch_offset) as usize;

    Ok(())
}
