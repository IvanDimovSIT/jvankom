use std::{num::NonZeroUsize, rc::Rc};

use crate::{
    bytecode::{
        method_descriptor_parser::{parse_descriptor, pop_params, pop_params_for_special},
        read_u16_from_bytecode, validate_cp_index,
    },
    class_file::{ClassFile, MethodAccessFlags},
    initialise_class_and_rewind,
    jvm_cache::method_call_cache::{
        InterfaceMethodCallInfo, StaticMethodCallInfo, VirtualMethodCallInfo,
    },
    jvm_model::{
        HeapObject, JvmClass, JvmContext, JvmError, JvmResult, JvmStackFrame, JvmValue,
        OBJECT_CLASS_NAME,
    },
    throw_incompatible_class_change_error, throw_null_pointer_exception,
    v_table::VTableEntry,
    validate_access,
};

pub fn invoke_static_or_special_instruction<const IS_SPECIAL: bool>(
    context: JvmContext,
) -> JvmResult<()> {
    const INSTRUCTION_SIZE: usize = 3;
    let frame = context.current_thread.top_frame();
    let unvalidated_cp_index = read_u16_from_bytecode(frame);
    let current_class = frame.class.clone();

    // check cache
    if let Some(call_info) = current_class
        .state
        .borrow()
        .cache
        .get_static_method(unvalidated_cp_index)
    {
        let params = if IS_SPECIAL {
            if let Some(params) = pop_params_for_special(&call_info.parameter_list, frame)? {
                params
            } else {
                throw_null_pointer_exception!(frame, context, INSTRUCTION_SIZE);
            }
        } else {
            pop_params(&call_info.parameter_list, frame)?
        };

        return invoke_method(
            context,
            call_info.bytecode_index,
            call_info.method_index,
            call_info.class.clone(),
            params,
        );
    };

    // cache miss: resolve method
    // find and load class
    let cp_index = validate_cp_index(unvalidated_cp_index)?;
    let current_class = frame.class.clone();
    let (class_name, method_name, method_descriptor) = current_class.read_method_ref(cp_index)?;

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
        throw_incompatible_class_change_error!(frame, context, INSTRUCTION_SIZE);
    }
    validate_access!(
        frame.class,
        loaded_class,
        called_method.access_flags,
        frame,
        context,
        INSTRUCTION_SIZE
    );

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

    if !loaded_class.state.borrow().is_initialised {
        initialise_class_and_rewind!(frame, context, &loaded_class, INSTRUCTION_SIZE);
    }

    // call resolved method
    let params = if IS_SPECIAL {
        if let Some(params) = pop_params_for_special(&param_types, frame)? {
            params
        } else {
            throw_null_pointer_exception!(frame, context, INSTRUCTION_SIZE);
        }
    } else {
        pop_params(&param_types, frame)?
    };

    invoke_method(
        context,
        called_bytecode_index,
        called_method_index,
        loaded_class,
        params,
    )
}

pub fn invoke_virtual_instruction(context: JvmContext) -> JvmResult<()> {
    const INSTRUCTION_SIZE: usize = 3;
    let frame = context.current_thread.top_frame();
    let unvalidated_cp_index = read_u16_from_bytecode(frame);
    let current_class = frame.class.clone();

    let current_class_state_ref = current_class.state.borrow();
    let (method_signature_id, params) = if let Some(virtual_cache) = current_class_state_ref
        .cache
        .get_virtual_method(unvalidated_cp_index)
    {
        if let Some(params) = pop_params_for_special(&virtual_cache.parameter_list, frame)? {
            (virtual_cache.method_signature_id, params)
        } else {
            throw_null_pointer_exception!(frame, context, INSTRUCTION_SIZE);
        }
    } else {
        drop(current_class_state_ref);
        let cp_index = validate_cp_index(unvalidated_cp_index)?;
        let (class_name, method_name, method_descriptor) =
            current_class.read_method_ref(cp_index)?;

        let called_class = context.class_loader.get(class_name)?;
        let param_types = parse_descriptor(method_descriptor)?;
        let method_signature_id = context
            .cache
            .method_signature_cache
            .get_id(method_name, method_descriptor);
        let virtual_call_info = VirtualMethodCallInfo {
            method_signature_id,
            parameter_list: param_types.clone(),
        };
        context.cache.method_call_cache.register_virtual_call_info(
            virtual_call_info,
            unvalidated_cp_index,
            &current_class,
        );

        if !called_class.state.borrow().is_initialised {
            initialise_class_and_rewind!(frame, context, &called_class, INSTRUCTION_SIZE);
        }

        let params = if let Some(params) = pop_params_for_special(&param_types, frame)? {
            params
        } else {
            throw_null_pointer_exception!(frame, context, INSTRUCTION_SIZE);
        };

        (method_signature_id, params)
    };
    let object_ref = match params[0] {
        JvmValue::Reference(Some(non_zero)) => non_zero,
        _ => unreachable!("type and null already checked"),
    };
    let called_object = context.heap.get(object_ref);

    let object_class = match called_object {
        HeapObject::Object { class, fields: _ } => class.clone(),
        _ => context.class_loader.get(OBJECT_CLASS_NAME)?,
    };

    let v_table_entry =
        if let Some(v_table_entry) = object_class.state.borrow().v_table.get(method_signature_id) {
            v_table_entry
        } else {
            let cp_index = validate_cp_index(unvalidated_cp_index)?;
            let (_class_name, method_name, method_descriptor) =
                current_class.read_method_ref(cp_index)?;
            let v_table_entry = find_virtual_method(&object_class, method_name, method_descriptor)?;
            object_class
                .state
                .borrow_mut()
                .v_table
                .register(method_signature_id, v_table_entry.clone());
            v_table_entry
        };
    let called_method =
        &v_table_entry.resolved_class.class_file.methods[v_table_entry.method_index];
    if called_method
        .access_flags
        .check_flag(MethodAccessFlags::STATIC_FLAG)
    {
        throw_incompatible_class_change_error!(frame, context, INSTRUCTION_SIZE);
    }
    validate_access!(
        frame.class,
        v_table_entry.resolved_class,
        called_method.access_flags,
        frame,
        context,
        INSTRUCTION_SIZE
    );

    invoke_method(
        context,
        v_table_entry.bytecode_index,
        v_table_entry.method_index,
        v_table_entry.resolved_class,
        params,
    )
}

pub fn invoke_interface(context: JvmContext) -> JvmResult<()> {
    const INSTRUCTION_SIZE: usize = 5;
    let frame = context.current_thread.top_frame();
    let (index, _count) = read_invokeinterface_params(frame)?;
    let current_class = frame.class.clone();
    let current_class_state = current_class.state.borrow();
    let (interface, method_signature_id, params) =
        if let Some(call_info) = current_class_state.cache.get_interface_method(index) {
            if let Some(params) = pop_params_for_special(&call_info.parameter_list, frame)? {
                (
                    call_info.interface.clone(),
                    call_info.method_signature_id,
                    params,
                )
            } else {
                throw_null_pointer_exception!(frame, context, INSTRUCTION_SIZE);
            }
        } else {
            drop(current_class_state);
            let (interface_name, method_name, method_desc) =
                current_class.read_interface_method_ref(index)?;

            let interface = context.class_loader.get(interface_name)?;
            if !interface.state.borrow().is_initialised {
                initialise_class_and_rewind!(frame, context, &interface, INSTRUCTION_SIZE);
            }

            let param_types = parse_descriptor(method_desc)?;
            let method_signature_id = context
                .cache
                .method_signature_cache
                .get_id(method_name, method_desc);
            let call_info = InterfaceMethodCallInfo {
                interface: interface.clone(),
                method_signature_id,
                parameter_list: param_types.clone(),
            };
            context
                .cache
                .method_call_cache
                .register_interface_call_info(call_info, index, &current_class);

            let params = if let Some(params) = pop_params_for_special(&param_types, frame)? {
                params
            } else {
                throw_null_pointer_exception!(frame, context, INSTRUCTION_SIZE);
            };

            (interface, method_signature_id, params)
        };

    let object_ref = match params[0] {
        JvmValue::Reference(Some(non_zero)) => non_zero,
        _ => unreachable!("type and null already checked"),
    };
    let called_object = context.heap.get(object_ref);

    let object_class = match called_object {
        HeapObject::Object { class, fields: _ } => class.clone(),
        _ => context.class_loader.get(OBJECT_CLASS_NAME)?,
    };

    if !JvmClass::is_sublcass_of(&interface, &object_class) {
        throw_incompatible_class_change_error!(frame, context, INSTRUCTION_SIZE);
    }

    let v_table_entry =
        if let Some(v_table_entry) = object_class.state.borrow().v_table.get(method_signature_id) {
            v_table_entry
        } else {
            let (_class_name, method_name, method_descriptor) =
                current_class.read_interface_method_ref(index)?;
            let v_table_entry = find_virtual_method(&object_class, method_name, method_descriptor)?;
            object_class
                .state
                .borrow_mut()
                .v_table
                .register(method_signature_id, v_table_entry.clone());
            v_table_entry
        };

    let called_method =
        &v_table_entry.resolved_class.class_file.methods[v_table_entry.method_index];
    if called_method
        .access_flags
        .check_flag(MethodAccessFlags::STATIC_FLAG)
    {
        throw_incompatible_class_change_error!(frame, context, INSTRUCTION_SIZE);
    }
    validate_access!(
        frame.class,
        v_table_entry.resolved_class,
        called_method.access_flags,
        frame,
        context,
        INSTRUCTION_SIZE
    );

    invoke_method(
        context,
        v_table_entry.bytecode_index,
        v_table_entry.method_index,
        v_table_entry.resolved_class,
        params,
    )
}

fn read_invokeinterface_params(frame: &mut JvmStackFrame) -> JvmResult<(NonZeroUsize, usize)> {
    let bytecode =
        frame.class.class_file.methods[frame.method_index].get_bytecode(frame.bytecode_index);

    let index_byte1 = bytecode.code[frame.program_counter] as usize;
    frame.program_counter += 1;
    debug_assert!(frame.program_counter < bytecode.code.len());

    let index_byte2 = bytecode.code[frame.program_counter] as usize;
    frame.program_counter += 1;
    debug_assert!(frame.program_counter < bytecode.code.len());

    let index = if let Some(ind) = NonZeroUsize::new((index_byte1 << 8) | index_byte2) {
        ind
    } else {
        return Err(JvmError::InvalidConstantPoolIndex.bx());
    };

    let count = bytecode.code[frame.program_counter] as usize;
    frame.program_counter += 1;
    debug_assert!(frame.program_counter < bytecode.code.len());

    let _zero_reserved = bytecode.code[frame.program_counter] as usize;
    frame.program_counter += 1;
    debug_assert!(frame.program_counter < bytecode.code.len());
    debug_assert_eq!(0, _zero_reserved);

    Ok((index, count))
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
            break;
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

    if let Some(entry) = search_class_inehritence_chain_for_default_interface_method(
        object_class,
        method_name,
        method_descriptor,
    ) {
        return Ok(entry);
    }

    Err(construct_virtual_method_error(
        method_name,
        method_descriptor,
    ))
}

fn search_class_inehritence_chain_for_default_interface_method(
    object_class: &Rc<JvmClass>,
    method_name: &str,
    method_descriptor: &str,
) -> Option<VTableEntry> {
    if let Some(entry) = search_interfaces_for_default_method(
        &object_class.state.borrow().interfaces,
        method_name,
        method_descriptor,
    ) {
        return Some(entry);
    }
    object_class.class_file.super_class_index?;

    let mut parent_class = object_class
        .state
        .borrow()
        .super_class
        .clone()
        .expect("Parent class not loaded");

    loop {
        if let Some(entry) = search_interfaces_for_default_method(
            &parent_class.state.borrow().interfaces,
            method_name,
            method_descriptor,
        ) {
            return Some(entry);
        }

        parent_class.class_file.super_class_index?;
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

fn search_interfaces_for_default_method(
    interfaces: &[Rc<JvmClass>],
    method_name: &str,
    method_descriptor: &str,
) -> Option<VTableEntry> {
    for interface in interfaces {
        if let Some((method, bytecode_index)) = interface
            .class_file
            .get_method_and_bytecode_index(method_name, method_descriptor)
            && check_not_abstract_method(&interface.class_file, method)
        {
            return Some(VTableEntry::new(interface.clone(), method, bytecode_index));
        }

        if let Some(entry) = search_interfaces_for_default_method(
            &interface.state.borrow().interfaces,
            method_name,
            method_descriptor,
        ) {
            return Some(entry);
        }
    }

    None
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

fn check_not_abstract_method(class_file: &ClassFile, method_index: usize) -> bool {
    !class_file.methods[method_index]
        .access_flags
        .check_flag(MethodAccessFlags::ABSTRACT_FLAG)
}

fn invoke_method(
    context: JvmContext,
    bytecode_index: Option<usize>,
    method_index: usize,
    class: Rc<JvmClass>,
    params: Vec<JvmValue>,
) -> JvmResult<()> {
    if let Some(bytecode_index) = bytecode_index {
        let new_frame = JvmStackFrame::new(class, method_index, bytecode_index, params);

        context.current_thread.push(new_frame);
        Ok(())
    } else {
        let called_method = &class.class_file.methods[method_index];
        if !called_method
            .access_flags
            .check_flag(MethodAccessFlags::NATIVE_FLAG)
        {
            return Err(JvmError::ExpectedNativeMethod {
                method_name: class
                    .class_file
                    .constant_pool
                    .expect_utf8(called_method.name_index)
                    .to_owned(),
                method_descriptor: class
                    .class_file
                    .constant_pool
                    .expect_utf8(called_method.descriptor_index)
                    .to_owned(),
            }
            .bx());
        }

        context.native_method_resolver.execute_native_method(
            context.current_thread,
            context.heap,
            context.class_loader,
            params,
            method_index,
            class,
        )
    }
}
