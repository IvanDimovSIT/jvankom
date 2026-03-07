use crate::{throw_array_index_out_of_bounds_exception, throw_null_pointer_exception};

use super::*;

#[inline]
fn generic_load_n<F, T>(context: JvmContext, expect_fn: F) -> JvmResult<()>
where
    F: FnOnce(JvmValue) -> JvmResult<T>,
{
    let frame = context.current_thread.top_frame();
    let index_value = read_u8_from_bytecode(frame) as usize;

    if let Some(value) = frame.local_variables.get(index_value) {
        expect_fn(*value)?;
        frame.operand_stack.push(*value);
    } else {
        return Err(JvmError::NoLocalVariableFound.bx());
    }

    Ok(())
}

#[inline]
fn generic_load<const INDEX: usize, F, T>(context: JvmContext, expect_fn: F) -> JvmResult<()>
where
    F: FnOnce(JvmValue) -> JvmResult<T>,
{
    let frame = context.current_thread.top_frame();

    if let Some(value) = frame.local_variables.get(INDEX) {
        expect_fn(*value)?;
        frame.operand_stack.push(*value);
    } else {
        return Err(JvmError::NoLocalVariableFound.bx());
    }

    Ok(())
}

#[inline]
fn generic_load_array_instruction<U, W, T>(
    context: JvmContext,
    unwrap_array: U,
    wrap_value: W,
) -> JvmResult<()>
where
    U: FnOnce(&mut HeapObject) -> JvmResult<&mut Vec<T>>,
    W: FnOnce(T) -> JvmValue,
    T: Copy,
{
    let frame = context.current_thread.top_frame();

    let index = pop_int(frame)?;
    let array_ref = if let Some(array_ref) = pop_reference(frame)? {
        array_ref
    } else {
        throw_null_pointer_exception!(frame, context, 1);
    };

    let array = unwrap_array(context.heap.get(array_ref))?;

    if index < 0 || index as usize >= array.len() {
        throw_array_index_out_of_bounds_exception!(frame, context, 1);
    }
    let value = array[index as usize];
    frame.operand_stack.push(wrap_value(value));

    Ok(())
}

pub fn integer_load_n(context: JvmContext) -> JvmResult<()> {
    generic_load_n(context, expect_int)
}

pub fn integer_load<const INDEX: usize>(context: JvmContext) -> JvmResult<()> {
    generic_load::<INDEX, _, _>(context, expect_int)
}

pub fn reference_load_instruction<const INDEX: usize>(context: JvmContext) -> JvmResult<()> {
    generic_load::<INDEX, _, _>(context, expect_reference)
}

pub fn reference_load_n(context: JvmContext) -> JvmResult<()> {
    generic_load_n(context, expect_reference)
}

pub fn load_integer_array_instruction(context: JvmContext) -> JvmResult<()> {
    generic_load_array_instruction(
        context,
        |obj| match obj {
            HeapObject::IntArray(items) => Ok(items),
            _ => Err(JvmError::IncompatibleArrayType.bx()),
        },
        JvmValue::Int,
    )
}

pub fn load_character_array_instruction(context: JvmContext) -> JvmResult<()> {
    generic_load_array_instruction(
        context,
        |obj| match obj {
            HeapObject::CharacterArray(items) => Ok(items),
            _ => Err(JvmError::IncompatibleArrayType.bx()),
        },
        |char| JvmValue::Int(char as i32),
    )
}
