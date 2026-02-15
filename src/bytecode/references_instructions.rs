use std::rc::Rc;

use crate::{
    bytecode::{
        method_descriptor_parser::{parse_descriptor, pop_params},
        object_field_initialisation::{determine_field_types, initialise_object_fields},
    },
    class_file::MethodAccessFlags,
    jvm::JVM,
    method_call_cache::StaticMethodCallInfo,
    object_instantiation_cache::ObjectInstantiationInfo,
};

use super::*;

pub fn new_array_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
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

    let array_ref = context.heap.allocate(object);
    frame
        .operand_stack
        .push(JvmValue::Reference(Some(array_ref)));

    Ok(())
}

pub fn invoke_static_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let unvalidated_cp_index = read_u16_from_bytecode(frame);

    // check cache
    if let Some(call_info) = context
        .cache
        .method_call_cache
        .get_static_call_info(&frame.class, unvalidated_cp_index)
    {
        let params = pop_params(&call_info.parameter_list, frame)?;
        let new_frame = JvmStackFrame::new(
            call_info.class.clone(),
            call_info.method_index,
            call_info.bytecode_index,
            params,
        );

        context.current_thread.push(new_frame);

        return Ok(());
    };

    // cache miss: resolve method
    // find and load class
    let cp_index = if let Some(index) = NonZeroUsize::new(unvalidated_cp_index as usize) {
        index
    } else {
        return Err(JvmError::InvalidConstantPoolIndex.bx());
    };
    let current_class = frame.class.clone();
    let (class_name, method_name, method_descriptor) = if let Some(called_method) = current_class
        .constant_pool
        .get_class_methodname_descriptor(cp_index)
    {
        called_method
    } else {
        return Err(JvmError::InvalidMethodRefIndex(cp_index).bx());
    };

    let loaded_class = context.class_loader.get(class_name)?;
    let (called_method_index, called_bytecode_index) = if let Some(index) = loaded_class
        .class
        .get_method_and_bytecode_index(method_name, method_descriptor)
    {
        index
    } else {
        return Err(JvmError::MethodNotFound {
            class_name: class_name.to_owned(),
            method_name: method_name.to_owned(),
        }
        .bx());
    };

    // check access
    let called_method = &loaded_class.class.methods[called_method_index];
    if !called_method
        .access_flags
        .check_flag(MethodAccessFlags::STATIC_FLAG)
    {
        todo!("Throw IncopatibleClassChangeError")
    }
    if called_method
        .access_flags
        .check_flag(MethodAccessFlags::PRIVATE_FLAG)
        && Rc::as_ptr(&loaded_class.class) != Rc::as_ptr(&frame.class)
    {
        todo!("Throw IllegalAccessError")
    }

    // register method in cache
    let param_types = parse_descriptor(method_descriptor)?;

    let static_method_call_info = StaticMethodCallInfo {
        class: loaded_class.class.clone(),
        method_index: called_method_index,
        bytecode_index: called_bytecode_index,
        parameter_list: param_types.clone(),
    };

    context.cache.method_call_cache.register_static_call_info(
        static_method_call_info,
        unvalidated_cp_index,
        &frame.class,
    );

    // initialise class and rewind
    if !loaded_class.is_initialised {
        frame.program_counter -= 3;
        JVM::initialise_class(
            context.current_thread,
            &loaded_class,
            context.class_loader,
            class_name,
        )?;

        return Ok(());
    }

    // call resolved method
    let params = pop_params(&param_types, frame)?;

    let new_frame = JvmStackFrame::new(
        loaded_class.class.clone(),
        called_method_index,
        called_bytecode_index,
        params,
    );

    context.current_thread.push(new_frame);

    Ok(())
}

pub fn new_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let unvalidated_cp_index = read_u16_from_bytecode(frame);

    // check cache
    if let Some(instance_info) = context
        .cache
        .object_instantiation_cache
        .get_object_instantiation_info(&frame.class, unvalidated_cp_index)
    {
        let object =
            initialise_object_fields(instance_info.class.clone(), &instance_info.field_infos);
        let object_ref = context.heap.allocate(object);
        frame
            .operand_stack
            .push(JvmValue::Reference(Some(object_ref)));

        return Ok(());
    }

    // find and load class
    let cp_index = if let Some(index) = NonZeroUsize::new(unvalidated_cp_index as usize) {
        index
    } else {
        return Err(JvmError::InvalidConstantPoolIndex.bx());
    };
    let current_class = frame.class.clone();
    let class_name = if let Some(name) = current_class.constant_pool.get_class_name(cp_index) {
        name
    } else {
        return Err(JvmError::InvalidClassIndex(cp_index).bx());
    };

    let loaded_class = context.class_loader.get(class_name)?;

    // initialise class and rewind
    if !loaded_class.is_initialised {
        frame.program_counter -= 3;
        JVM::initialise_class(
            context.current_thread,
            &loaded_class,
            context.class_loader,
            class_name,
        )?;

        return Ok(());
    }

    let field_infos = if let Some(instantiation_info) = context
        .cache
        .object_instantiation_cache
        .get_object_instatiation_info_from_class(&loaded_class.class)
    {
        instantiation_info.field_infos.clone()
    } else {
        determine_field_types(&loaded_class.class, context.class_loader)?
    };

    let object = initialise_object_fields(loaded_class.class.clone(), &field_infos);
    let object_ref = context.heap.allocate(object);
    frame
        .operand_stack
        .push(JvmValue::Reference(Some(object_ref)));

    let object_instantiation_info = ObjectInstantiationInfo {
        field_infos,
        class: loaded_class.class,
    };

    context
        .cache
        .object_instantiation_cache
        .register_object_instantiation_info(
            object_instantiation_info,
            &current_class,
            unvalidated_cp_index,
        );

    Ok(())
}
