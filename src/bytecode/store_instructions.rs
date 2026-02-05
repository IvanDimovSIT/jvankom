use super::*;

pub fn store_integer_array_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();

    let value = pop_int(frame)?;
    let index = pop_int(frame)?;
    let array_ref = if let Some(array_ref) = pop_reference(frame)? {
        array_ref
    } else {
        todo!("Throw NullPointerException");
    };

    let array = if let Some(array) = context.heap.get(array_ref) {
        match array {
            HeapObject::IntArray(items) => items,
            _ => todo!("Throw ArrayStoreException"),
        }
    } else {
        return Err(JvmError::InvalidReference.bx());
    };

    if index < 0 || index as usize >= array.len() {
        todo!("Throw ArrayIndexOutOfBoundsException");
    }
    array[index as usize] = value;

    Ok(())
}

pub fn store_reference_n_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let bytecode = frame.class.methods[frame.method_index].get_bytecode(frame.bytecode_index);
    let index_value = bytecode.code[frame.program_counter] as usize;
    frame.program_counter += 1;

    let reference = pop_reference(frame)?;

    frame.local_variables[index_value] = JvmValue::Reference(reference);

    Ok(())
}

pub fn store_reference_instruction<const INDEX: usize>(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();

    let reference = pop_reference(frame)?;
    debug_assert!(INDEX < frame.local_variables.len());
    frame.local_variables[INDEX] = JvmValue::Reference(reference);

    Ok(())
}
