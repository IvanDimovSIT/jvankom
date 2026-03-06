use std::rc::Rc;

use crate::{
    bytecode::method_descriptor_parser::{parse_descriptor, pop_params, pop_params_for_special},
    class_cache::{CacheEntry, FieldAccessInfo, TypeInfo},
    class_file::{ClassFile, FieldAccessFlags, MethodAccessFlags},
    class_loader::ClassLoader,
    field_initialisation::{determine_non_static_field_types, initialise_object_fields},
    initialise_class_and_rewind,
    jvm::JVM,
    jvm_model::{
        DescriptorType, JvmClass, OBJECT_CLASS_NAME, ObjectArray, ObjectArrayType, StaticFieldInfo,
    },
    method_call_cache::{StaticMethodCallInfo, VirtualMethodCallInfo},
    throw_negative_array_size_exception, throw_null_pointer_exception,
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
        throw_null_pointer_exception!(frame, context, 1);
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
        HeapObject::ObjectArray(arr) => arr.array.len(),
        _ => return Err(JvmError::ExpectedArray.bx()),
    } as i32;

    frame.operand_stack.push(JvmValue::Int(array_len));

    Ok(())
}

pub fn new_array_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let array_type_value = read_u8_from_bytecode(frame);

    let operand_value = pop_int(frame)?;
    if operand_value < 0 {
        throw_negative_array_size_exception!(frame, context, 2);
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
    let unvalidated_index = read_u16_from_bytecode(frame);
    let type_info = if let Some(info) = frame.class.state.borrow().cache.get_type(unvalidated_index)
    {
        info.clone()
    } else {
        let array_type_ref = validate_cp_index(unvalidated_index)?;

        let arr_type = if let Some(arr_type) = frame
            .class
            .class_file
            .constant_pool
            .get_class_name(array_type_ref)
        {
            arr_type
        } else {
            return Err(JvmError::InvalidClassIndex(array_type_ref).bx());
        };
        let (object_array_type, dimension) =
            determine_type_and_dimension_if_array(arr_type, context.class_loader)?;

        let type_info = TypeInfo {
            object_or_array: object_array_type,
            dimension: dimension,
        };

        frame.class.state.borrow_mut().cache.register(
            unvalidated_index,
            Rc::new(CacheEntry::Type(type_info.clone())),
        );

        if let ObjectArrayType::Class(jvm_class) = &type_info.object_or_array
            && !jvm_class.state.borrow().is_initialised
        {
            initialise_class_and_rewind!(frame, context, jvm_class, 3);
        }

        type_info
    };

    let (array_type, array_dimension) = determine_object_array_type_and_dimension(type_info)?;

    let operand_value = pop_int(frame)?;
    if operand_value < 0 {
        throw_negative_array_size_exception!(frame, context, 3);
    }
    let array_size = operand_value as usize;

    let object_array = ObjectArray {
        array: vec![None; array_size],
        dimension: array_dimension,
        object_array_type: array_type,
    };
    let object = HeapObject::ObjectArray(object_array);

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
                throw_null_pointer_exception!(frame, context, 3);
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
        initialise_class_and_rewind!(frame, context, &loaded_class, 3);
    }

    // call resolved method
    let params = if IS_SPECIAL {
        if let Some(params) = pop_params_for_special(&param_types, frame)? {
            params
        } else {
            throw_null_pointer_exception!(frame, context, 3);
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
        .get_type(unvalidated_cp_index)
    {
        match &new_object_class.object_or_array {
            ObjectArrayType::Class(jvm_class) => jvm_class.clone(),
            _ => {
                return Err(JvmError::InvalidClassIndex(
                    NonZeroUsize::new(unvalidated_cp_index as usize).unwrap(),
                )
                .bx());
            }
        }
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
            Rc::new(CacheEntry::Type(TypeInfo {
                object_or_array: ObjectArrayType::Class(loaded_class.clone()),
                dimension: 0,
            })),
        );

        if !loaded_class.state.borrow().is_initialised {
            initialise_class_and_rewind!(frame, context, &loaded_class, 3);
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
            throw_null_pointer_exception!(frame, context, 3);
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

        if !called_class.state.borrow().is_initialised {
            initialise_class_and_rewind!(frame, context, &called_class, 3);
        }

        let params = if let Some(params) = pop_params_for_special(&param_types, frame)? {
            params
        } else {
            throw_null_pointer_exception!(frame, context, 3);
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
        throw_null_pointer_exception!(frame, context, 3);
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
        throw_null_pointer_exception!(frame, context, 3);
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
        throw_null_pointer_exception!(frame, context, 1);
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
    let type_info = if let Some(type_info) = frame
        .class
        .state
        .borrow()
        .cache
        .get_type(unvalidated_cp_index)
    {
        type_info.clone()
    } else {
        let class_index = validate_cp_index(unvalidated_cp_index)?;

        let current_class = frame.class.clone();
        let class_name = current_class
            .class_file
            .constant_pool
            .get_class_name(class_index)
            .expect("Should be validated");

        let (expected_class, dimension_if_array) =
            determine_type_and_dimension_if_array(class_name, context.class_loader)?;

        let type_info = TypeInfo {
            object_or_array: expected_class,
            dimension: dimension_if_array,
        };

        frame.class.state.borrow_mut().cache.register(
            unvalidated_cp_index,
            Rc::new(CacheEntry::Type(type_info.clone())),
        );

        if let ObjectArrayType::Class(jvm_class) = &type_info.object_or_array
            && !jvm_class.state.borrow().is_initialised
        {
            initialise_class_and_rewind!(frame, context, jvm_class, 3);
        }

        type_info
    };
    let object_ref = if let Some(reference) = pop_reference(frame)? {
        reference
    } else {
        frame.operand_stack.push(JvmValue::Int(0));
        return Ok(());
    };
    let object = context.heap.get(object_ref);
    let instance_of_result =
        check_instance_of(&type_info.object_or_array, type_info.dimension, object)?;
    frame.operand_stack.push(JvmValue::Int(instance_of_result));

    Ok(())
}

/// convert type reference to array type
fn determine_object_array_type_and_dimension(
    type_info: TypeInfo,
) -> JvmResult<(ObjectArrayType, NonZeroUsize)> {
    match type_info.object_or_array {
        ObjectArrayType::Primitive(_) => {
            if type_info.dimension == 0 {
                Err(JvmError::InvalidMultidimensionalPrimitiveArrayDimension.bx())
            } else {
                Ok((
                    type_info.object_or_array,
                    NonZeroUsize::new(type_info.dimension + 1).unwrap(),
                ))
            }
        }
        _ => Ok((
            type_info.object_or_array,
            NonZeroUsize::new(type_info.dimension + 1).unwrap(),
        )),
    }
}

/// determines the type based on a class/interface reference and it's dimension if it's an array (0 if not array)
fn determine_type_and_dimension_if_array(
    object_type: &str,
    class_loader: &mut ClassLoader,
) -> JvmResult<(ObjectArrayType, usize)> {
    if !object_type.starts_with('[') {
        let class_type = class_loader.get(object_type)?;
        return Ok((ObjectArrayType::Class(class_type), 0));
    }

    // for multidimensional arrays:
    let inner_type = object_type.trim_start_matches('[');
    let dimension = object_type.len() - inner_type.len();
    if inner_type.starts_with('L') {
        let inner_type_class = class_loader.get(&inner_type[1..(inner_type.len() - 1)])?;
        Ok((ObjectArrayType::Class(inner_type_class), dimension))
    } else {
        let primitive_type = inner_type
            .chars()
            .next()
            .expect("Excepted primitive type")
            .into();

        Ok((ObjectArrayType::Primitive(primitive_type), dimension))
    }
}

fn check_instance_of(
    expected_type: &ObjectArrayType,
    dimension_if_array: usize,
    object: &HeapObject,
) -> JvmResult<i32> {
    let matches = match object {
        HeapObject::Object { class, fields: _ } => {
            check_non_array_instance_of(class, expected_type, dimension_if_array)
        }
        HeapObject::IntArray(_) => {
            check_array_instance_of(DescriptorType::Integer, expected_type, dimension_if_array)
        }
        HeapObject::ByteArray(_) => {
            check_array_instance_of(DescriptorType::Byte, expected_type, dimension_if_array)
        }
        HeapObject::BooleanArray(_) => {
            check_array_instance_of(DescriptorType::Boolean, expected_type, dimension_if_array)
        }
        HeapObject::CharacterArray(_) => {
            check_array_instance_of(DescriptorType::Character, expected_type, dimension_if_array)
        }
        HeapObject::ShortArray(_) => {
            check_array_instance_of(DescriptorType::Short, expected_type, dimension_if_array)
        }
        HeapObject::FloatArray(_) => {
            check_array_instance_of(DescriptorType::Float, expected_type, dimension_if_array)
        }
        HeapObject::DoubleArray(_) => {
            check_array_instance_of(DescriptorType::Double, expected_type, dimension_if_array)
        }
        HeapObject::LongArray(_) => {
            check_array_instance_of(DescriptorType::Long, expected_type, dimension_if_array)
        }
        HeapObject::ObjectArray(object_array) => {
            check_object_array_instance_of(object_array, expected_type, dimension_if_array)
        }
    };

    let int = if matches { 1 } else { 0 };

    Ok(int)
}

fn check_object_array_instance_of(
    obj_array: &ObjectArray,
    expected_type: &ObjectArrayType,
    expected_dimension: usize,
) -> bool {
    let types_match = match expected_type {
        ObjectArrayType::Class(ex_jvm_class) => {
            if expected_dimension == 0 && ex_jvm_class.class_file.super_class_index.is_none() {
                return true;
            } else {
                match &obj_array.object_array_type {
                    ObjectArrayType::Class(jvm_class) => {
                        JvmClass::is_sublcass_of(ex_jvm_class, jvm_class)
                    }
                    _ => false,
                }
            }
        }
        ObjectArrayType::Primitive(ex_descriptor_type) => match &obj_array.object_array_type {
            ObjectArrayType::Primitive(descriptor_type) => descriptor_type == ex_descriptor_type,
            _ => false,
        },
    };
    let dimensions_match = expected_dimension == obj_array.dimension.get();
    types_match && dimensions_match
}

fn check_non_array_instance_of(
    obj_class: &Rc<JvmClass>,
    expected_type: &ObjectArrayType,
    expected_dimension: usize,
) -> bool {
    match expected_type {
        ObjectArrayType::Class(parent) => {
            expected_dimension == 0 && JvmClass::is_sublcass_of(parent, obj_class)
        }
        _ => false,
    }
}

fn check_array_instance_of(
    array_type: DescriptorType,
    expected_type: &ObjectArrayType,
    expected_dimension: usize,
) -> bool {
    let types_match = match expected_type {
        ObjectArrayType::Primitive(descriptor_type) => *descriptor_type == array_type,
        ObjectArrayType::Class(jvm_class) => {
            if jvm_class.class_file.super_class_index.is_none() && 0 == expected_dimension {
                return true;
            } else {
                false
            }
        }
    };

    expected_dimension == 1 && types_match
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

    if !class.state.borrow().is_initialised {
        initialise_class_and_rewind!(frame, context, &class, 3);
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
        todo!("Throw access exception and rewind by 2")
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
        todo!("Throw access error and rewind pc by 2")
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
