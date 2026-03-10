use std::rc::Rc;

use crate::{
    class_file::ConstantValue,
    field_initialisation::{determine_non_static_field_types, initialise_object_fields},
    initialise_class_and_rewind,
    jvm_model::{CLASS_CLASS_NAME, JvmClass, STRING_CLASS_NAME},
};

use super::*;

pub fn nop_instruction(_context: JvmContext) -> JvmResult<()> {
    Ok(())
}

pub fn null_const_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.top_frame();
    frame.operand_stack.push(JvmValue::Reference(None));

    Ok(())
}

pub fn integer_const_instruction<const VALUE: i32>(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.top_frame();
    frame.operand_stack.push(JvmValue::Int(VALUE));

    Ok(())
}

pub fn bipush_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.top_frame();
    let read_value = read_u8_from_bytecode(frame);
    // sign-extend the value
    let push_value = (read_value as i8) as i32;
    frame.operand_stack.push(JvmValue::Int(push_value));

    Ok(())
}

pub fn sipush_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.top_frame();
    // sign-extend the value
    let push_value = (read_u16_from_bytecode(frame) as i16) as i32;

    frame.operand_stack.push(JvmValue::Int(push_value));

    Ok(())
}

pub fn ldc2w_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.top_frame();
    let bytecode_value = read_u16_from_bytecode(frame);

    let constant_index = validate_cp_index(bytecode_value)?;

    let value = match frame.class.class_file.constant_pool.get(constant_index) {
        ConstantValue::Long(long) => JvmValue::Long(*long),
        ConstantValue::Double(double) => JvmValue::Double(*double),
        _ => return Err(JvmError::InvalidConstantPoolIndex.bx()),
    };

    frame.operand_stack.push(value);

    Ok(())
}

pub fn ldc_instruction(context: JvmContext) -> JvmResult<()> {
    const INSTRUCTION_SIZE: usize = 2;
    let frame = context.current_thread.top_frame();
    let bytecode_value = read_u8_from_bytecode(frame) as u16;
    let constant_index = validate_cp_index(bytecode_value)?;

    let value = match frame.class.class_file.constant_pool.get(constant_index) {
        ConstantValue::Int(int) => JvmValue::Int(*int),
        ConstantValue::Float(float) => JvmValue::Float(*float),
        ConstantValue::Class { name_index } => {
            let class_class = context.class_loader.get(CLASS_CLASS_NAME)?;

            if !class_class.state.borrow().is_initialised {
                initialise_class_and_rewind!(frame, context, &class_class, INSTRUCTION_SIZE);
            }

            let class_name = frame
                .class
                .class_file
                .constant_pool
                .get_utf8(*name_index)
                .expect("Should be validated");

            let obj = create_class_object(&class_class, class_name)?;
            let class_obj_ref = context.heap.allocate(obj);

            JvmValue::Reference(Some(class_obj_ref))
        }
        ConstantValue::String { utf8_index } => {
            let string_class = context.class_loader.get(STRING_CLASS_NAME)?;

            // initialise class and rewind
            if !string_class.state.borrow().is_initialised {
                initialise_class_and_rewind!(frame, context, &string_class, INSTRUCTION_SIZE);
            }

            let mut string_obj = create_string_object(&string_class)?;
            let text = frame
                .class
                .class_file
                .constant_pool
                .get_utf8(*utf8_index)
                .expect("Should be validated");

            context
                .cache
                .string_pool
                .initialise_string_fields(text, &mut string_obj, context.heap);

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

fn create_string_object(string_class: &Rc<JvmClass>) -> JvmResult<HeapObject> {
    debug_assert_eq!(STRING_CLASS_NAME, string_class.class_file.get_class_name());

    if let Some(str) = string_class.state.borrow().default_object.clone() {
        Ok(str)
    } else {
        let non_static_field_types = determine_non_static_field_types(string_class)?;
        let str = initialise_object_fields(string_class.clone(), &non_static_field_types);
        let mut state = string_class.state.borrow_mut();
        state.non_static_fields = Some(non_static_field_types);
        state.default_object = Some(str.clone());

        Ok(str)
    }
}

fn create_class_object(class_class: &Rc<JvmClass>, _class_name: &str) -> JvmResult<HeapObject> {
    debug_assert_eq!(CLASS_CLASS_NAME, class_class.class_file.get_class_name());

    if let Some(cl) = class_class.state.borrow().default_object.clone() {
        Ok(cl)
    } else {
        let non_static_field_types = determine_non_static_field_types(class_class)?;
        let str = initialise_object_fields(class_class.clone(), &non_static_field_types);
        let mut state = class_class.state.borrow_mut();
        state.non_static_fields = Some(non_static_field_types);
        state.default_object = Some(str.clone());

        Ok(str)
    }

    // TODO: initilaise Class object
}
