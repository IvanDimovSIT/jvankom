use super::*;

#[inline]
fn store_generic_array_instruction<P, F, T>(
    context: JvmContext,
    pop_generic: P,
    get_array_fn: F,
) -> JvmResult<()>
where
    P: FnOnce(&mut JvmStackFrame) -> JvmResult<T>,
    F: FnOnce(&mut HeapObject) -> &mut Vec<T>,
{
    let frame = context.current_thread.peek().unwrap();

    let value = pop_generic(frame)?;
    let index = pop_int(frame)?;
    let array_ref = if let Some(array_ref) = pop_reference(frame)? {
        array_ref
    } else {
        todo!("Throw NullPointerException");
    };

    let array = get_array_fn(context.heap.get(array_ref));

    if index < 0 || index as usize >= array.len() {
        todo!("Throw ArrayIndexOutOfBoundsException");
    }
    array[index as usize] = value;

    Ok(())
}

pub fn store_integer_array_instruction(context: JvmContext) -> JvmResult<()> {
    store_generic_array_instruction(context, pop_int, |obj| match obj {
        HeapObject::IntArray(items) => items,
        _ => todo!("Throw ArrayStoreException"),
    })
}

pub fn store_character_array_instruction(context: JvmContext) -> JvmResult<()> {
    store_generic_array_instruction(
        context,
        |frame| Ok(pop_int(frame)? as u16),
        |obj| match obj {
            HeapObject::CharacterArray(items) => items,
            _ => todo!("Throw ArrayStoreException"),
        },
    )
}

pub fn store_object_array_instruction(context: JvmContext) -> JvmResult<()> {
    //TODO: check for matching types
    store_generic_array_instruction(context, pop_reference, |obj| match obj {
        HeapObject::ObjectArray(items) => items,
        _ => todo!("Throw ArrayStoreException"),
    })
}

#[inline]
fn store_generic_n_instruction<P, W, T>(
    context: JvmContext,
    pop_generic: P,
    wrap_value: W,
) -> JvmResult<()>
where
    P: FnOnce(&mut JvmStackFrame) -> JvmResult<T>,
    W: FnOnce(T) -> JvmValue,
{
    let frame = context.current_thread.peek().unwrap();
    let bytecode =
        frame.class.class_file.methods[frame.method_index].get_bytecode(frame.bytecode_index);
    let index_value = bytecode.code[frame.program_counter] as usize;
    frame.program_counter += 1;

    let generic_value = pop_generic(frame)?;

    frame.local_variables[index_value] = wrap_value(generic_value);

    Ok(())
}

#[inline]
fn store_generic_instruction<const INDEX: usize, P, W, T>(
    context: JvmContext,
    pop_generic: P,
    wrap_value: W,
) -> JvmResult<()>
where
    P: FnOnce(&mut JvmStackFrame) -> JvmResult<T>,
    W: FnOnce(T) -> JvmValue,
{
    let frame = context.current_thread.peek().unwrap();

    let value = pop_generic(frame)?;
    debug_assert!(INDEX < frame.local_variables.len());
    frame.local_variables[INDEX] = wrap_value(value);

    Ok(())
}

pub fn store_reference_n_instruction(context: JvmContext) -> JvmResult<()> {
    store_generic_n_instruction(context, pop_reference, JvmValue::Reference)
}

pub fn store_reference_instruction<const INDEX: usize>(context: JvmContext) -> JvmResult<()> {
    store_generic_instruction::<INDEX, _, _, _>(context, pop_reference, JvmValue::Reference)
}

pub fn store_integer_n_instruction(context: JvmContext) -> JvmResult<()> {
    store_generic_n_instruction(context, pop_int, JvmValue::Int)
}

pub fn store_integer_instruction<const INDEX: usize>(context: JvmContext) -> JvmResult<()> {
    store_generic_instruction::<INDEX, _, _, _>(context, pop_int, JvmValue::Int)
}
