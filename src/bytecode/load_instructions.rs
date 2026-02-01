use super::*;

pub fn integer_load_n(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> JvmResult<()> {
    let frame = thread.peek().unwrap();
    let bytecode = frame.class.methods[frame.method_index].get_bytecode(frame.bytecode_index);
    let index_value = bytecode.code[frame.program_counter] as usize;
    frame.program_counter += 1;

    if let Some(value) = frame.local_variables.get(index_value) {
        expect_int(*value)?;
        frame.operand_stack.push(*value);
    } else {
        return Err(JvmError::NoLocalVariableFound.bx());
    }

    Ok(())
}

pub fn integer_load<const INDEX: usize>(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> JvmResult<()> {
    let frame = thread.peek().unwrap();

    if let Some(value) = frame.local_variables.get(INDEX) {
        expect_int(*value)?;
        frame.operand_stack.push(*value);
    } else {
        return Err(JvmError::NoLocalVariableFound.bx());
    }

    Ok(())
}

pub fn reference_load_instruction<const INDEX: usize>(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> JvmResult<()> {
    let frame = thread.peek().unwrap();

    if let Some(value) = frame.local_variables.get(INDEX) {
        expect_reference(*value)?;
        frame.operand_stack.push(*value);
    } else {
        return Err(JvmError::NoLocalVariableFound.bx());
    }

    Ok(())
}

pub fn reference_load_n(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> JvmResult<()> {
    let frame = thread.peek().unwrap();
    let bytecode = frame.class.methods[frame.method_index].get_bytecode(frame.bytecode_index);
    let index_value = bytecode.code[frame.program_counter] as usize;
    frame.program_counter += 1;

    if let Some(value) = frame.local_variables.get(index_value) {
        expect_reference(*value)?;
        frame.operand_stack.push(*value);
    } else {
        return Err(JvmError::NoLocalVariableFound.bx());
    }

    Ok(())
}

pub fn load_integer_array_instruction(
    thread: &mut JvmThread,
    heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> JvmResult<()> {
    let frame = thread.peek().unwrap();

    let index = pop_int(frame)?;
    let array_ref = if let Some(array_ref) = pop_reference(frame)? {
        array_ref
    } else {
        todo!("Throw NullPointerException");
    };

    let array = if let Some(array) = heap.get(array_ref) {
        match array {
            HeapObject::IntArray(items) => items,
            _ => return Err(JvmError::IncompatibleArrayType.bx()),
        }
    } else {
        return Err(JvmError::InvalidReference.bx());
    };

    if index < 0 || index as usize >= array.len() {
        todo!("Throw ArrayIndexOutOfBoundsException");
    }
    let value = array[index as usize];
    frame.operand_stack.push(JvmValue::Int(value));

    Ok(())
}
