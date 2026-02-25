use crate::{
    class_file::ConstantValue,
    field_initialisation::{determine_non_static_field_types, initialise_object_fields},
    jvm::JVM,
    jvm_heap::JvmHeap,
};

use super::*;

const STRING_CLASS_NAME: &str = "java/lang/String";

pub fn nop_instruction(_context: JvmContext) -> JvmResult<()> {
    Ok(())
}

pub fn null_const_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    frame.operand_stack.push(JvmValue::Reference(None));

    Ok(())
}

pub fn integer_const_instruction<const VALUE: i32>(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    frame.operand_stack.push(JvmValue::Int(VALUE));

    Ok(())
}

pub fn bipush_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let bytecode =
        frame.class.class_file.methods[frame.method_index].get_bytecode(frame.bytecode_index);
    // sign-extend the value
    let push_value = (bytecode.code[frame.program_counter] as i8) as i32;
    frame.program_counter += 1;

    frame.operand_stack.push(JvmValue::Int(push_value));

    Ok(())
}

pub fn sipush_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    // sign-extend the value
    let push_value = (read_u16_from_bytecode(frame) as i16) as i32;

    frame.operand_stack.push(JvmValue::Int(push_value));

    Ok(())
}

pub fn ldc_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let bytecode =
        frame.class.class_file.methods[frame.method_index].get_bytecode(frame.bytecode_index);

    let bytecode_value = bytecode.code[frame.program_counter] as u16;
    frame.program_counter += 1;

    let constant_index = validate_cp_index(bytecode_value)?;

    let value = match frame.class.class_file.constant_pool.get(constant_index) {
        ConstantValue::Int(int) => JvmValue::Int(*int),
        ConstantValue::Float(_) => unimplemented!(),
        ConstantValue::Class { name_index } => unimplemented!(),
        ConstantValue::String { utf8_index } => {
            let string_class = context.class_loader.get(STRING_CLASS_NAME)?;

            // initialise class and rewind
            if !string_class.state.borrow().is_initialised {
                frame.program_counter -= 2;
                JVM::initialise_class(
                    context.current_thread,
                    &string_class,
                    context.class_loader,
                    STRING_CLASS_NAME,
                )?;
                return Ok(());
            }

            let mut string_obj = if let Some(str) =
                string_class.state.borrow().default_object.clone()
            {
                str
            } else {
                let non_static_field_types = determine_non_static_field_types(&string_class)?;
                let str = initialise_object_fields(string_class.clone(), &non_static_field_types);
                string_class.state.borrow_mut().default_object = Some(str.clone());

                str
            };
            let text = frame
                .class
                .class_file
                .constant_pool
                .get_utf8(*utf8_index)
                .expect("Should be validated");

            initialise_string_object(&mut string_obj, text, context.heap);

            let reference = context.heap.allocate(string_obj);

            JvmValue::Reference(Some(reference))
        }
        ConstantValue::MethodRef {
            class_index,
            name_and_type_index,
        } => unimplemented!(),
        ConstantValue::InterfaceMethodRef {
            class_index,
            name_and_type_index,
        } => unimplemented!(),
        ConstantValue::MethodHandle {
            reference_kind,
            reference_index,
        } => unimplemented!(),
        ConstantValue::MethodType { descriptor_index } => unimplemented!(),
        _ => return Err(JvmError::InvalidConstantPoolIndex.bx()),
    };

    frame.operand_stack.push(value);

    Ok(())
}

fn initialise_string_object(obj: &mut HeapObject, text: &str, heap: &mut JvmHeap) {
    // TODO: implement string initialisation
    println!("ERROR: initialise_string_object not implemented")
}
