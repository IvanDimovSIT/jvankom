use std::rc::Rc;

use crate::{
    bytecode::{
        method_descriptor_parser::{parse_descriptor, pop_params, pop_params_for_virtual},
        object_field_initialisation::{determine_field_types, initialise_object_fields},
    },
    class_file::{ClassFile, MethodAccessFlags},
    class_loader::ClassLoader,
    jvm::JVM,
    jvm_model::JvmHeap,
    method_call_cache::{StaticMethodCallInfo, VirtualMethodCallInfo},
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
    let cp_index = validate_cp_index(unvalidated_cp_index)?;
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
    let cp_index = validate_cp_index(unvalidated_cp_index)?;
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

pub fn invoke_virtual_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let unvalidated_cp_index = read_u16_from_bytecode(frame);
    let current_class = frame.class.clone();

    let cp_index = validate_cp_index(unvalidated_cp_index)?;
    let (class_name, method_name, method_descriptor) = if let Some(called_method) = current_class
        .constant_pool
        .get_class_methodname_descriptor(cp_index)
    {
        called_method
    } else {
        return Err(JvmError::InvalidMethodRefIndex(cp_index).bx());
    };

    let called_class = context.class_loader.get(class_name)?;

    // initialise class and rewind
    if !called_class.is_initialised {
        frame.program_counter -= 3;
        JVM::initialise_class(
            context.current_thread,
            &called_class,
            context.class_loader,
            class_name,
        )?;

        return Ok(());
    }

    let object_ref = if let Some(reference) = pop_reference(frame)? {
        reference
    } else {
        todo!("Throw NullPointerException");
    };
    let called_object = context
        .heap
        .get(object_ref)
        .ok_or_else(|| JvmError::InvalidReference.bx())?;
    let object_class = match called_object {
        HeapObject::Object { class, fields: _ } => class.clone(),
        _ => context.class_loader.get("java/lang/Object")?.class,
    };

    if let Some(info) = context.cache.method_call_cache.get_virtual_call_info(
        &object_class,
        method_name,
        method_descriptor,
    ) {
        let called_method = &info.resolved_class.methods[info.method_index];
        if called_method
            .access_flags
            .check_flag(MethodAccessFlags::PRIVATE_FLAG)
            && Rc::as_ptr(&info.resolved_class) != Rc::as_ptr(&frame.class)
        {
            todo!("Throw IllegalAccessError")
        }
        let params = pop_params_for_virtual(object_ref, &info.types, frame)?;

        let new_frame = JvmStackFrame::new(
            info.resolved_class.clone(),
            info.method_index,
            info.bytecode_index,
            params,
        );

        context.current_thread.push(new_frame);

        return Ok(());
    }

    let (resolved_class, called_method_index, called_bytecode_index) = find_virtual_method(
        &object_class,
        context.class_loader,
        method_name,
        method_descriptor,
    )?;
    let called_method = &resolved_class.methods[called_method_index];
    if called_method
        .access_flags
        .check_flag(MethodAccessFlags::STATIC_FLAG)
    {
        todo!("Throw IncopatibleClassChangeError")
    }
    if called_method
        .access_flags
        .check_flag(MethodAccessFlags::PRIVATE_FLAG)
        && Rc::as_ptr(&resolved_class) != Rc::as_ptr(&frame.class)
    {
        todo!("Throw IllegalAccessError")
    }

    let param_types = parse_descriptor(method_descriptor)?;
    let virtual_method_call_info = VirtualMethodCallInfo {
        bytecode_index: called_bytecode_index,
        method_index: called_method_index,
        resolved_class: resolved_class.clone(),
        types: param_types.clone(),
    };
    context.cache.method_call_cache.register_virtual_call_info(
        &object_class,
        method_name,
        method_descriptor,
        virtual_method_call_info,
    );

    let params = pop_params_for_virtual(object_ref, &param_types, frame)?;

    let new_frame = JvmStackFrame::new(
        resolved_class,
        called_method_index,
        called_bytecode_index,
        params,
    );

    context.current_thread.push(new_frame);

    Ok(())
}

/// returns classfile + method index + bytecode index
fn find_virtual_method(
    object_class: &Rc<ClassFile>,
    class_loader: &mut ClassLoader,
    method_name: &str,
    method_descriptor: &str,
) -> JvmResult<(Rc<ClassFile>, usize, usize)> {
    if let Some((method, bytecode_index)) =
        object_class.get_method_and_bytecode_index(method_name, method_descriptor)
    {
        return Ok((object_class.clone(), method, bytecode_index));
    }

    let parent_name = if let Some(parent_name) = object_class.get_super_class_name() {
        parent_name
    } else {
        return Err(construct_virtual_method_error(
            method_name,
            method_descriptor,
        ));
    };
    let mut parent_class = class_loader.get(parent_name)?.class.clone();
    loop {
        if let Some((method, bytecode_index)) =
            parent_class.get_method_and_bytecode_index(method_name, method_descriptor)
        {
            return Ok((parent_class, method, bytecode_index));
        }

        let parent_class_name = if let Some(parent_class_name) = parent_class.get_super_class_name()
        {
            parent_class_name
        } else {
            return Err(construct_virtual_method_error(
                method_name,
                method_descriptor,
            ));
        };
        parent_class = class_loader.get(parent_class_name)?.class.clone();
    }
}

fn construct_virtual_method_error(
    method_name: impl Into<String>,
    method_descriptor: impl Into<String>,
) -> Box<JvmError> {
    JvmError::VirtualMethodError {
        method_name: method_name.into(),
        method_descriptor: method_descriptor.into(),
    }
    .bx()
}

fn validate_cp_index(unvalidated_cp_index: u16) -> JvmResult<NonZeroUsize> {
    if let Some(index) = NonZeroUsize::new(unvalidated_cp_index as usize) {
        Ok(index)
    } else {
        Err(JvmError::InvalidConstantPoolIndex.bx())
    }
}
