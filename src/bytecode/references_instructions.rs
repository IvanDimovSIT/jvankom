use crate::{
    bytecode::method_descriptor_parser::prepare_method_parameters, class_file::ConstantValue,
    jvm::JVM,
};

use super::*;

pub fn new_array_instruction(
    thread: &mut JvmThread,
    heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> JvmResult<()> {
    let frame = thread.peek().unwrap();
    let bytecode = frame.class.methods[frame.method_index].get_bytecode(frame.bytecode_index);
    let array_type_value = bytecode.code[frame.program_counter];
    frame.program_counter += 1;

    let operand_value = pop_int(frame)?;
    if operand_value < 0 {
        todo!("Throw NegativeArraySizeException");
    }
    let array_size = operand_value as usize;

    let object = match array_type_value {
        4 => HeapObject::BooleanArray(vec![false; array_size]),
        5 => HeapObject::CharacterArray(vec![0; array_size]),
        6 => HeapObject::FloatArray(vec![0.0; array_size]),
        7 => HeapObject::DoubleArray(vec![0.0; array_size]),
        8 => HeapObject::ByteArray(vec![0; array_size]),
        9 => HeapObject::ShortArray(vec![0; array_size]),
        10 => HeapObject::IntArray(vec![0; array_size]),
        11 => HeapObject::LongArray(vec![0; array_size]),
        _ => return Err(JvmError::InvalidArrayType(array_type_value).bx()),
    };

    let array_ref = heap.allocate(object);
    frame
        .operand_stack
        .push(JvmValue::Reference(Some(array_ref)));

    Ok(())
}

pub fn invoke_static_instruction(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    class_loader: &mut ClassLoader,
) -> JvmResult<()> {
    let frame = thread.peek().unwrap();
    let bytecode = frame.class.methods[frame.method_index].get_bytecode(frame.bytecode_index);
    let index_byte1 = bytecode.code[frame.program_counter] as usize;
    frame.program_counter += 1;
    let index_byte2 = bytecode.code[frame.program_counter] as usize;
    frame.program_counter += 1;

    let cp_index =
        NonZeroUsize::new((index_byte1 << 8) | index_byte2).expect("Method class index is invalid");
    let current_class = frame.class.clone();
    let (class_name, method_name, method_descriptor) = current_class
        .constant_pool
        .get_class_methodname_descriptor(cp_index)
        .expect("Method ref is invalid");

    let loaded_class = class_loader.get(class_name)?;
    let (called_method_index, called_bytecode_index) = if let Some(index) = loaded_class
        .class
        .get_method_and_bytecode_index(method_name)
    {
        index
    } else {
        return Err(JvmError::MethodNotFound {
            class_name: class_name.to_owned(),
            method_name: method_name.to_owned(),
        }
        .bx());
    };

    let params = prepare_method_parameters(frame, method_descriptor)?;
    let new_frame = JvmStackFrame::new(
        loaded_class.class.clone(),
        called_method_index,
        called_bytecode_index,
        params,
    );

    thread.push(new_frame);
    JVM::initialise_class(thread, &loaded_class)?;

    Ok(())
}
