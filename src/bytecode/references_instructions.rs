use std::rc::Rc;

use crate::{
    bytecode::method_descriptor_parser::{parse_descriptor, pop_params, pop_params_for_special},
    class_cache::{CacheEntry, FieldAccessInfo},
    class_file::{ClassFile, FieldAccessFlags, MethodAccessFlags},
    class_loader::ClassLoader,
    exceptions::{throw_negative_array_size_exception, throw_null_pointer_exception},
    field_initialisation::{determine_non_static_field_types, initialise_object_fields},
    jvm::JVM,
    jvm_model::{DescriptorType, JvmClass, OBJECT_CLASS_NAME, StaticFieldInfo},
    method_call_cache::{StaticMethodCallInfo, VirtualMethodCallInfo},
    v_table::VTableEntry,
};

use super::*;

const BOOLEAN_ARR: u8 = 4;
const CHAR_ARR: u8 = 5;
const FLOAT_ARR: u8 = 6;
const DOUBLE_ARR: u8 = 7;
const BYTE_ARR: u8 = 8;
const SHORT_ARR: u8 = 9;
const INT_ARR: u8 = 10;
const LONG_ARR: u8 = 11;

pub fn array_length_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let array_ref = if let Some(array_ref) = pop_reference(frame)? {
        array_ref
    } else {
        return throw_null_pointer_exception(context);
    };

    let array = context.heap.get(array_ref);
    let array_len = match array {
        HeapObject::IntArray(items) => items.len(),
        HeapObject::ByteArray(items) => items.len(),
        HeapObject::BooleanArray(items) => items.len(),
        HeapObject::CharacterArray(items) => items.len(),
        HeapObject::ShortArray(items) => items.len(),
        HeapObject::FloatArray(items) => items.len(),
        HeapObject::DoubleArray(items) => items.len(),
        HeapObject::LongArray(items) => items.len(),
        HeapObject::ObjectArray(items) => items.len(),
        _ => return Err(JvmError::ExpectedArray.bx()),
    } as i32;

    frame.operand_stack.push(JvmValue::Int(array_len));

    Ok(())
}

pub fn new_array_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let bytecode =
        frame.class.class_file.methods[frame.method_index].get_bytecode(frame.bytecode_index);
    let array_type_value = bytecode.code[frame.program_counter];
    frame.program_counter += 1;

    let operand_value = pop_int(frame)?;
    if operand_value < 0 {
        return throw_negative_array_size_exception(context);
    }
    let array_size = operand_value as usize;

    let object = match array_type_value {
        BOOLEAN_ARR => HeapObject::BooleanArray(vec![false; array_size]),
        CHAR_ARR => HeapObject::CharacterArray(vec![0; array_size]),
        FLOAT_ARR => HeapObject::FloatArray(vec![0.0; array_size]),
        DOUBLE_ARR => HeapObject::DoubleArray(vec![0.0; array_size]),
        BYTE_ARR => HeapObject::ByteArray(vec![0; array_size]),
        SHORT_ARR => HeapObject::ShortArray(vec![0; array_size]),
        INT_ARR => HeapObject::IntArray(vec![0; array_size]),
        LONG_ARR => HeapObject::LongArray(vec![0; array_size]),
        _ => return Err(JvmError::InvalidArrayType(array_type_value).bx()),
    };

    let array_ref = context.heap.allocate(object);
    frame
        .operand_stack
        .push(JvmValue::Reference(Some(array_ref)));

    Ok(())
}

pub fn new_object_array_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let array_type_ref = validate_cp_index(read_u16_from_bytecode(frame))?;

    //TODO: use and check arr_type
    let _arr_type = if let Some(arr_type) = frame
        .class
        .class_file
        .constant_pool
        .get_class_name(array_type_ref)
    {
        arr_type
    } else {
        return Err(JvmError::InvalidClassIndex(array_type_ref).bx());
    };

    let operand_value = pop_int(frame)?;
    if operand_value < 0 {
        return throw_negative_array_size_exception(context);
    }
    let array_size = operand_value as usize;

    let object = HeapObject::ObjectArray(vec![None; array_size]);

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
                return throw_null_pointer_exception(context);
            }
        } else {
            pop_params(&call_info.parameter_list, frame)?
        };

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
                context.class_loader,
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
    let params = if IS_SPECIAL {
        if let Some(params) = pop_params_for_special(&param_types, frame)? {
            params
        } else {
            return throw_null_pointer_exception(context);
        }
    } else {
        pop_params(&param_types, frame)?
    };

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
            context.class_loader,
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
        .cache
        .get_object_creation(unvalidated_cp_index)
    {
        new_object_class.clone()
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
        frame.class.state.borrow_mut().cache.register(
            unvalidated_cp_index,
            Rc::new(CacheEntry::ObjectCreation(loaded_class.clone())),
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

        loaded_class
    };

    let object = if let Some(object) = new_object_class.state.borrow().default_object.clone() {
        object
    } else {
        let field_infos = determine_non_static_field_types(&new_object_class)?;
        let object = initialise_object_fields(new_object_class.clone(), &field_infos);
        let mut state_ref = new_object_class.state.borrow_mut();
        state_ref.default_object = Some(object.clone());
        state_ref.non_static_fields = Some(field_infos);

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

    let current_class_state_ref = current_class.state.borrow();
    let (method_name, method_descriptor, params) = if let Some(virtual_cache) =
        current_class_state_ref
            .cache
            .get_virtual_method(unvalidated_cp_index)
    {
        if let Some(params) = pop_params_for_special(&virtual_cache.parameter_list, frame)? {
            (
                virtual_cache.method_name.as_str(),
                virtual_cache.descriptor.as_str(),
                params,
            )
        } else {
            return throw_null_pointer_exception(context);
        }
    } else {
        drop(current_class_state_ref);
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

        let params = if let Some(params) = pop_params_for_special(&param_types, frame)? {
            params
        } else {
            return throw_null_pointer_exception(context);
        };

        (method_name, method_descriptor, params)
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
            context.class_loader,
            params,
            v_table_entry.method_index,
            v_table_entry.resolved_class,
        );
    }

    Ok(())
}

pub fn get_field_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let unvalidated_cp_index = read_u16_from_bytecode(frame);
    let (field_index, _) = access_object_field(unvalidated_cp_index, frame, context.class_loader)?;
    let object_ref = if let Some(reference) = pop_reference(frame)? {
        reference
    } else {
        return throw_null_pointer_exception(context);
    };
    let (_object_class, object_fields) = match context.heap.get(object_ref) {
        HeapObject::Object { class, fields } => (class, fields),
        _ => todo!("Throw Exception: Expected non array"),
    };

    let value = object_fields[field_index];
    frame.operand_stack.push(value);

    Ok(())
}

pub fn put_field_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let unvalidated_cp_index = read_u16_from_bytecode(frame);
    let (field_index, field_descriptor) =
        access_object_field(unvalidated_cp_index, frame, context.class_loader)?;
    let value = pop_any(frame)?;
    let object_ref = if let Some(reference) = pop_reference(frame)? {
        reference
    } else {
        return throw_null_pointer_exception(context);
    };
    let (_object_class, object_fields) = match context.heap.get(object_ref) {
        HeapObject::Object { class, fields } => (class, fields),
        _ => todo!("Throw Exception: Expected non array"),
    };
    if !value.matches_type(field_descriptor) {
        todo!("Throw type error")
    }

    object_fields[field_index] = value;

    Ok(())
}

pub fn get_static_instruction(context: JvmContext) -> JvmResult<()> {
    generic_static_field_instruction(context, |field, frame| {
        frame.operand_stack.push(field.value);
        Ok(())
    })
}

pub fn put_static_instruction(context: JvmContext) -> JvmResult<()> {
    generic_static_field_instruction(context, |field, frame| {
        let descriptor = field.descriptor_type;
        let value = pop_any(frame)?;
        if !value.matches_type(descriptor) {
            todo!("Throw type error")
        }

        field.value = value;
        Ok(())
    })
}

pub fn throw_exception_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let exception_ref = if let Some(ex_ref) = pop_reference(frame)? {
        ex_ref
    } else {
        return throw_null_pointer_exception(context);
    };
    let exception_obj = context.heap.get(exception_ref);
    let exception_class = match exception_obj {
        HeapObject::Object { class, fields: _ } => class,
        _ => todo!("Throw exception - not an exception"),
    };
    //TODO: check exception class

    frame.set_exception(exception_ref);

    Ok(())
}

pub fn instance_of_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let unvalidated_cp_index = read_u16_from_bytecode(frame);
    //TODO: add cache check

    let class_index = validate_cp_index(unvalidated_cp_index)?;

    let current_class = frame.class.clone();
    let class_name = current_class
        .class_file
        .constant_pool
        .get_class_name(class_index)
        .expect("Should be validated");

    let object_ref = if let Some(reference) = pop_reference(frame)? {
        reference
    } else {
        return throw_null_pointer_exception(context);
    };
    let object = context.heap.get(object_ref);
    let instance_of_result = check_instance_of(class_name, object, context.class_loader)?;
    frame.operand_stack.push(JvmValue::Int(instance_of_result));

    Ok(())
}

fn check_instance_of(
    expected_name: &str,
    object: &HeapObject,
    class_loader: &mut ClassLoader,
) -> JvmResult<i32> {
    // TODO: implement array types - store array dimensions + class type for referece arr
    let int = match object {
        HeapObject::Object { class, fields: _ } => {
            if expected_name.starts_with('[') {
                0
            } else {
                let expected_class = class_loader.get(expected_name)?;
                if JvmClass::is_sublcass_of(&expected_class, class) {
                    1
                } else {
                    0
                }
            }
        }
        HeapObject::IntArray(_items) => {
            check_array_instance_of(DescriptorType::Integer, expected_name)
        }
        HeapObject::ByteArray(_items) => {
            check_array_instance_of(DescriptorType::Byte, expected_name)
        }
        HeapObject::BooleanArray(_items) => {
            check_array_instance_of(DescriptorType::Boolean, expected_name)
        }
        HeapObject::CharacterArray(_items) => {
            check_array_instance_of(DescriptorType::Reference, expected_name)
        }
        HeapObject::ShortArray(_items) => {
            check_array_instance_of(DescriptorType::Short, expected_name)
        }
        HeapObject::FloatArray(_items) => {
            check_array_instance_of(DescriptorType::Float, expected_name)
        }
        HeapObject::DoubleArray(_items) => {
            check_array_instance_of(DescriptorType::Double, expected_name)
        }
        HeapObject::LongArray(_items) => {
            check_array_instance_of(DescriptorType::Long, expected_name)
        }
        HeapObject::ObjectArray(_non_zeros) => {
            check_array_instance_of(DescriptorType::Reference, expected_name)
        }
    };

    Ok(int)
}

fn check_array_instance_of(array_type: DescriptorType, expected_name: &str) -> i32 {
    if expected_name == OBJECT_CLASS_NAME {
        return 1;
    } else if expected_name.len() < 2 {
        return 0;
    }

    let expected_name = &expected_name[0..2];
    let matches = match array_type {
        DescriptorType::Integer => expected_name == "[I",
        DescriptorType::Long => expected_name == "[J",
        DescriptorType::Reference => expected_name == "[[" || expected_name == "[L",
        DescriptorType::Short => expected_name == "[S",
        DescriptorType::Character => expected_name == "[C",
        DescriptorType::Byte => expected_name == "[B",
        DescriptorType::Float => expected_name == "[F",
        DescriptorType::Double => expected_name == "[D",
        DescriptorType::Boolean => expected_name == "[Z",
    };

    if matches { 1 } else { 0 }
}

#[inline]
fn generic_static_field_instruction<F>(context: JvmContext, field_fn: F) -> JvmResult<()>
where
    F: FnOnce(&mut StaticFieldInfo, &mut JvmStackFrame) -> JvmResult<()>,
{
    let frame = context.current_thread.peek().unwrap();
    let unvalidated_cp_index = read_u16_from_bytecode(frame);
    let current_class = frame.class.clone();

    // cache hit:
    let current_class_state = current_class.state.borrow();
    if let Some(info) = current_class_state
        .cache
        .get_static_field_access(unvalidated_cp_index)
        .cloned()
    {
        drop(current_class_state);
        let mut state = info.target_class.state.borrow_mut();
        let fields = state
            .static_fields
            .as_mut()
            .expect("Fields should be initialised");
        return field_fn(&mut fields[info.field_index], frame);
    }
    drop(current_class_state);

    // cache miss:
    let cp_index = validate_cp_index(unvalidated_cp_index)?;
    let (class_name, field_name, _descriptor) = if let Some(static_field) = current_class
        .class_file
        .constant_pool
        .get_field_class_name_type(cp_index)
    {
        static_field
    } else {
        return Err(JvmError::InvalidMethodRefIndex(cp_index).bx());
    };

    let class = context.class_loader.get(class_name)?;

    // initialise class and rewind
    if !class.state.borrow().is_initialised {
        frame.program_counter -= 3;
        JVM::initialise_class(
            context.current_thread,
            &class,
            context.class_loader,
            class_name,
        )?;

        return Ok(());
    }

    let (class, index) = find_static_field_class_with_index(class, field_name)?;
    let mut state = class.state.borrow_mut();
    let fields = state
        .static_fields
        .as_mut()
        .expect("Static fields not initialised");

    let class_file_index = fields[index].field_class_file_index;
    if class.class_file.fields[class_file_index]
        .access_flags
        .check_flag(FieldAccessFlags::PRIVATE_FLAG)
        && Rc::as_ptr(&current_class) != Rc::as_ptr(&class)
    {
        todo!("Throw access exception")
    }

    field_fn(&mut fields[index], frame)?;
    drop(state);

    let info = FieldAccessInfo {
        target_class: class,
        field_index: index,
    };
    current_class.state.borrow_mut().cache.register(
        unvalidated_cp_index,
        Rc::new(CacheEntry::StaticFieldAccess(info)),
    );

    Ok(())
}

fn find_static_field_class_with_index(
    class: Rc<JvmClass>,
    name: &str,
) -> JvmResult<(Rc<JvmClass>, usize)> {
    let class_state = class.state.borrow();
    if let Some(static_fields) = &class_state.static_fields {
        for (index, field) in static_fields.iter().enumerate() {
            if name == field.name {
                drop(class_state);
                return Ok((class, index));
            }
        }

        if let Some(parent) = &class_state.super_class {
            let parent = parent.clone();
            drop(class_state);
            return find_static_field_class_with_index(parent, name);
        }
    }

    Err(JvmError::StaticFieldNotFound {
        class_name: class
            .class_file
            .get_class_name()
            .unwrap_or_default()
            .to_owned(),
        field_name: name.to_owned(),
    }
    .bx())
}

fn access_object_field(
    unvalidated_cp_index: u16,
    frame: &mut JvmStackFrame,
    class_loader: &mut ClassLoader,
) -> JvmResult<(usize, DescriptorType)> {
    let current_class = frame.class.clone();

    // check cache
    if let Some(info) = current_class
        .state
        .borrow()
        .cache
        .get_non_static_field_access(unvalidated_cp_index)
    {
        let field_descriptor =
            get_non_static_field_descriptor(&info.target_class, info.field_index);

        return Ok((info.field_index, field_descriptor));
    }

    // cache miss
    let cp_index = validate_cp_index(unvalidated_cp_index)?;
    let (field_class_name, field_name, _field_type) = if let Some(field_info) = current_class
        .class_file
        .constant_pool
        .get_field_class_name_type(cp_index)
    {
        field_info
    } else {
        return Err(JvmError::InvalidMethodRefIndex(cp_index).bx());
    };
    let declared_class = class_loader.get(field_class_name)?;
    let field_index = find_field_index(&declared_class, field_name)?;
    let declared_class_file_field_index = declared_class
        .state
        .borrow()
        .non_static_fields
        .as_ref()
        .expect("Missing non static fields")[field_index]
        .field_class_file_index;

    // the class that originally declared the field (the top parent that has the field)
    let field_class = declared_class
        .state
        .borrow()
        .non_static_fields
        .as_ref()
        .expect("Missing non static fields")[field_index]
        .class
        .clone();
    if field_class.class_file.fields[declared_class_file_field_index]
        .access_flags
        .check_flag(FieldAccessFlags::PRIVATE_FLAG)
        && Rc::as_ptr(&declared_class) != Rc::as_ptr(&current_class)
    {
        todo!("Throw access error")
    }
    let field_descriptor = get_non_static_field_descriptor(&declared_class, field_index);

    let field_access_info = FieldAccessInfo {
        target_class: declared_class.clone(),
        field_index,
    };
    current_class.state.borrow_mut().cache.register(
        unvalidated_cp_index,
        Rc::new(CacheEntry::NonStaticFieldAccess(field_access_info)),
    );

    Ok((field_index, field_descriptor))
}

#[inline]
fn get_non_static_field_descriptor(
    class: &JvmClass,
    non_static_field_index: usize,
) -> DescriptorType {
    class
        .state
        .borrow()
        .non_static_fields
        .as_ref()
        .expect("Missing non static fields")[non_static_field_index]
        .descriptor_type
}

fn find_field_index(declared_class: &Rc<JvmClass>, field_name: &str) -> JvmResult<usize> {
    let decl_state = declared_class.state.borrow();

    if let Some(fields) = &decl_state.non_static_fields {
        for (i, field) in fields.iter().enumerate() {
            if field.name == field_name && Rc::ptr_eq(&field.class, declared_class) {
                return Ok(i);
            }
        }

        for (i, field) in fields.iter().enumerate() {
            if field.name == field_name {
                return Ok(i);
            }
        }
    } else {
        drop(decl_state);
        let field_types = determine_non_static_field_types(declared_class)?;
        declared_class.state.borrow_mut().non_static_fields = Some(field_types);
        return find_field_index(declared_class, field_name);
    }

    Err(JvmError::FieldNotFound {
        class_name: declared_class
            .class_file
            .get_class_name()
            .unwrap_or_default()
            .to_owned(),
        field_name: field_name.to_owned(),
    }
    .bx())
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
