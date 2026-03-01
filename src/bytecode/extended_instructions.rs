use super::*;

pub fn if_null_instruction(context: JvmContext) -> JvmResult<()> {
    generic_if_reference_instruction(context, |r| r.is_none())
}

pub fn if_not_null_instruction(context: JvmContext) -> JvmResult<()> {
    generic_if_reference_instruction(context, |r| r.is_some())
}

#[inline]
fn generic_if_reference_instruction<F>(context: JvmContext, logic_fn: F) -> JvmResult<()>
where
    F: FnOnce(Option<NonZeroUsize>) -> bool,
{
    let frame = context.current_thread.peek().unwrap();
    let reference = pop_reference(frame)?;
    if !logic_fn(reference) {
        // skip branch location
        frame.program_counter += 2;
        return Ok(());
    }

    let instruction_start = frame.program_counter as isize - 1;
    let branch_offset = read_i16_from_bytecode(frame) as isize;
    frame.program_counter = (instruction_start + branch_offset) as usize;

    Ok(())
}
