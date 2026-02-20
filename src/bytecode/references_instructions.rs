use std::rc::Rc;

use crate::{
    bytecode::{
        method_descriptor_parser::{parse_descriptor, pop_params, pop_params_for_special},
        object_field_initialisation::{determine_non_static_field_types, initialise_object_fields},
    },
    class_file::{ClassFile, MethodAccessFlags},
    jvm::JVM,
    jvm_model::{DescriptorType, JvmClass},
    method_call_cache::{StaticMethodCallInfo, VirtualMethodCallInfo},
    v_table::VTableEntry,
};

use super::*;

pub fn new_array_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let bytecode =
        frame.class.class_file.methods[frame.method_index].get_bytecode(frame.bytecode_index);
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

pub fn invoke_static_or_special_instruction<const IS_SPECIAL: bool>(
    context: JvmContext,
) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let unvalidated_cp_index = read_u16_from_bytecode(frame);

    // check cache
    if let Some(call_info) = context
        .cache
        .method_call_cache
        .get_static_call_info(&frame.class, unvalidated_cp_index)
    {
        let params =
            pop_params_for_static_or_special::<IS_SPECIAL>(&call_info.parameter_list, frame)?;
        if let Some(bytecode_index) = call_info.bytecode_index {
            let new_frame = JvmStackFrame::new(
                call_info.class.clone(),
                call_info.method_index,
                bytecode_index,
                params,
            );

            context.current_thread.push(new_frame);
        } else {
            return context.native_method_resolver.execute_native_method(
                context.current_thread,
                context.heap,
                params,
                call_info.method_index,
                call_info.class.clone(),
            );
        }

        return Ok(());
    };

    // cache miss: resolve method
    // find and load class
    let cp_index = validate_cp_index(unvalidated_cp_index)?;
    let current_class = frame.class.clone();
    let (class_name, method_name, method_descriptor) = if let Some(called_method) = current_class
        .class_file
        .constant_pool
        .get_class_methodname_descriptor(cp_index)
    {
        called_method
    } else {
        return Err(JvmError::InvalidMethodRefIndex(cp_index).bx());
    };

    let loaded_class = context.class_loader.get(class_name)?;
    let (called_method_index, called_bytecode_index) = if let Some(index) = loaded_class
        .class_file
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
    let called_method = &loaded_class.class_file.methods[called_method_index];
    if IS_SPECIAL
        == called_method
            .access_flags
            .check_flag(MethodAccessFlags::STATIC_FLAG)
    {
        todo!("Throw IncopatibleClassChangeError")
    }
    if called_method
        .access_flags
        .check_flag(MethodAccessFlags::PRIVATE_FLAG)
        && Rc::as_ptr(&loaded_class) != Rc::as_ptr(&frame.class)
    {
        todo!("Throw IllegalAccessError")
    }

    // register method in cache
    let param_types = parse_descriptor(method_descriptor)?;

    let static_method_call_info = StaticMethodCallInfo {
        class: loaded_class.clone(),
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
    if !loaded_class.state.borrow().is_initialised {
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
    let params = pop_params_for_static_or_special::<IS_SPECIAL>(&param_types, frame)?;

    if let Some(bytecode_index) = called_bytecode_index {
        let new_frame =
            JvmStackFrame::new(loaded_class, called_method_index, bytecode_index, params);

        context.current_thread.push(new_frame);
    } else {
        if !called_method
            .access_flags
            .check_flag(MethodAccessFlags::NATIVE_FLAG)
        {
            todo!("should be native method")
        }

        return context.native_method_resolver.execute_native_method(
            context.current_thread,
            context.heap,
            params,
            called_method_index,
            loaded_class,
        );
    }

    Ok(())
}

pub fn new_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let unvalidated_cp_index = read_u16_from_bytecode(frame);
    let new_object_class = if let Some(new_object_class) = frame
        .class
        .state
        .borrow()
        .object_creation_cache
        .get(unvalidated_cp_index)
    {
        new_object_class
    } else {
        // find and load class
        let cp_index = validate_cp_index(unvalidated_cp_index)?;
        let current_class = frame.class.clone();
        let class_name = if let Some(name) = current_class
            .class_file
            .constant_pool
            .get_class_name(cp_index)
        {
            name
        } else {
            return Err(JvmError::InvalidClassIndex(cp_index).bx());
        };

        let loaded_class = context.class_loader.get(class_name)?;
        frame
            .class
            .state
            .borrow_mut()
            .object_creation_cache
            .register(unvalidated_cp_index, loaded_class.clone());

        // initialise class and rewind
        if !loaded_class.state.borrow().is_initialised {
            frame.program_counter -= 3;
            JVM::initialise_class(
                context.current_thread,
                &loaded_class,
                context.class_loader,
                class_name,
            )?;

            return Ok(());
        }

        loaded_class
    };

    let object = if let Some(object) = new_object_class.state.borrow().default_object.clone() {
        object
    } else {
        let field_infos = determine_non_static_field_types(&new_object_class)?;
        let object = initialise_object_fields(new_object_class.clone(), &field_infos);
        new_object_class.state.borrow_mut().default_object = Some(object.clone());
        new_object_class.state.borrow_mut().non_static_fields = Some(field_infos);

        object
    };

    let object_ref = context.heap.allocate(object);
    frame
        .operand_stack
        .push(JvmValue::Reference(Some(object_ref)));

    Ok(())
}

pub fn invoke_virtual_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let unvalidated_cp_index = read_u16_from_bytecode(frame);
    let current_class = frame.class.clone();

    let (method_name, method_descriptor, params) = if let Some(virtual_cache) = context
        .cache
        .method_call_cache
        .get_virtual_call_info(&frame.class, unvalidated_cp_index)
    {
        (
            virtual_cache.method_name.as_str(),
            virtual_cache.descriptor.as_str(),
            pop_params_for_special(&virtual_cache.parameter_list, frame)?,
        )
    } else {
        let cp_index = validate_cp_index(unvalidated_cp_index)?;
        let (class_name, method_name, method_descriptor) = if let Some(called_method) =
            current_class
                .class_file
                .constant_pool
                .get_class_methodname_descriptor(cp_index)
        {
            called_method
        } else {
            return Err(JvmError::InvalidMethodRefIndex(cp_index).bx());
        };

        let called_class = context.class_loader.get(class_name)?;
        let param_types = parse_descriptor(method_descriptor)?;
        let virtual_call_info = VirtualMethodCallInfo {
            method_name: method_name.to_owned(),
            descriptor: method_descriptor.to_owned(),
            parameter_list: param_types.clone(),
        };
        context.cache.method_call_cache.register_virtual_call_info(
            virtual_call_info,
            unvalidated_cp_index,
            &current_class,
        );

        // initialise class and rewind
        if !called_class.state.borrow().is_initialised {
            frame.program_counter -= 3;
            JVM::initialise_class(
                context.current_thread,
                &called_class,
                context.class_loader,
                class_name,
            )?;

            return Ok(());
        }

        let params = pop_params_for_special(&param_types, frame)?;

        (method_name, method_descriptor, params)
    };
    let object_ref = match params[0] {
        JvmValue::Reference(Some(non_zero)) => non_zero,
        _ => unreachable!("type and null already checked"),
    };
    let called_object = context
        .heap
        .get(object_ref)
        .ok_or_else(|| JvmError::InvalidReference.bx())?;

    let object_class = match called_object {
        HeapObject::Object { class, fields: _ } => class.clone(),
        _ => context.class_loader.get("java/lang/Object")?,
    };

    let v_table_entry = if let Some(v_table_entry) = object_class
        .state
        .borrow()
        .v_table
        .get(method_name, method_descriptor)
    {
        v_table_entry
    } else {
        let v_table_entry = find_virtual_method(&object_class, method_name, method_descriptor)?;
        object_class.state.borrow_mut().v_table.register(
            method_name,
            method_descriptor,
            v_table_entry.clone(),
        );
        v_table_entry
    };
    let called_method =
        &v_table_entry.resolved_class.class_file.methods[v_table_entry.method_index];
    if called_method
        .access_flags
        .check_flag(MethodAccessFlags::STATIC_FLAG)
    {
        todo!("Throw IncopatibleClassChangeError")
    }
    if called_method
        .access_flags
        .check_flag(MethodAccessFlags::PRIVATE_FLAG)
        && Rc::as_ptr(&v_table_entry.resolved_class) != Rc::as_ptr(&frame.class)
    {
        todo!("Throw IllegalAccessError")
    }

    if let Some(bytecode_index) = v_table_entry.bytecode_index {
        let new_frame = JvmStackFrame::new(
            v_table_entry.resolved_class,
            v_table_entry.method_index,
            bytecode_index,
            params,
        );

        context.current_thread.push(new_frame);
    } else {
        if !called_method
            .access_flags
            .check_flag(MethodAccessFlags::NATIVE_FLAG)
        {
            todo!("should be native method")
        }

        return context.native_method_resolver.execute_native_method(
            context.current_thread,
            context.heap,
            params,
            v_table_entry.method_index,
            v_table_entry.resolved_class,
        );
    }

    Ok(())
}

fn check_not_abstract_method(class_file: &ClassFile, method_index: usize) -> bool {
    !class_file.methods[method_index]
        .access_flags
        .check_flag(MethodAccessFlags::ABSTRACT_FLAG)
}

/// returns class + method index + bytecode index
fn find_virtual_method(
    object_class: &Rc<JvmClass>,
    method_name: &str,
    method_descriptor: &str,
) -> JvmResult<VTableEntry> {
    if let Some((method, bytecode_index)) = object_class
        .class_file
        .get_method_and_bytecode_index(method_name, method_descriptor)
        && check_not_abstract_method(&object_class.class_file, method)
    {
        return Ok(VTableEntry::new(
            object_class.clone(),
            method,
            bytecode_index,
        ));
    }

    if object_class.class_file.super_class_index.is_none() {
        return Err(construct_virtual_method_error(
            method_name,
            method_descriptor,
        ));
    };
    let mut parent_class = object_class
        .state
        .borrow()
        .super_class
        .clone()
        .expect("Parent class not loaded");
    loop {
        if let Some((method, bytecode_index)) = parent_class
            .class_file
            .get_method_and_bytecode_index(method_name, method_descriptor)
            && check_not_abstract_method(&parent_class.class_file, method)
        {
            return Ok(VTableEntry::new(parent_class, method, bytecode_index));
        }

        if parent_class.class_file.super_class_index.is_none() {
            return Err(construct_virtual_method_error(
                method_name,
                method_descriptor,
            ));
        };
        parent_class = {
            parent_class
                .state
                .borrow()
                .super_class
                .clone()
                .expect("Parent class not loaded")
        };
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

fn pop_params_for_static_or_special<const IS_SPECIAL: bool>(
    types: &[DescriptorType],
    frame: &mut JvmStackFrame,
) -> JvmResult<Vec<JvmValue>> {
    if IS_SPECIAL {
        pop_params_for_special(types, frame)
    } else {
        pop_params(types, frame)
    }
}
