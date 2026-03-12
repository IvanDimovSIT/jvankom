use crate::{
    bytecode::{
        access_check::check_has_access, pop_any, pop_reference, read_u16_from_bytecode,
        validate_cp_index,
    },
    class_cache::{CacheEntry, FieldAccessInfo},
    class_loader::ClassLoader,
    field_initialisation::determine_non_static_field_types,
    initialise_class_and_rewind,
    jvm_model::{
        DescriptorType, HeapObject, JvmClass, JvmContext, JvmError, JvmResult, JvmStackFrame,
        StaticFieldInfo,
    },
    throw_illegal_access_error, throw_null_pointer_exception, validate_access,
};
use std::rc::Rc;

pub fn get_field_instruction(context: JvmContext) -> JvmResult<()> {
    const INSTRUCTION_SIZE: usize = 3;
    let frame = context.current_thread.top_frame();
    let unvalidated_cp_index = read_u16_from_bytecode(frame);
    let field_index = if let AccessObjectFieldResult::HasAccess {
        field_index,
        field_descriptor: _,
    } = access_object_field(unvalidated_cp_index, frame, context.class_loader)?
    {
        field_index
    } else {
        throw_illegal_access_error!(frame, context, INSTRUCTION_SIZE);
    };
    let object_ref = if let Some(reference) = pop_reference(frame)? {
        reference
    } else {
        throw_null_pointer_exception!(frame, context, INSTRUCTION_SIZE);
    };
    let object_fields = match context.heap.get(object_ref) {
        HeapObject::Object { class: _, fields } => fields,
        _ => todo!("Throw Exception: Expected non array"),
    };

    let value = object_fields[field_index];
    frame.operand_stack.push(value);

    Ok(())
}

pub fn put_field_instruction(context: JvmContext) -> JvmResult<()> {
    const INSTRUCTION_SIZE: usize = 3;
    let frame = context.current_thread.top_frame();
    let unvalidated_cp_index = read_u16_from_bytecode(frame);
    let (field_index, field_descriptor) = if let AccessObjectFieldResult::HasAccess {
        field_index,
        field_descriptor,
    } =
        access_object_field(unvalidated_cp_index, frame, context.class_loader)?
    {
        (field_index, field_descriptor)
    } else {
        throw_illegal_access_error!(frame, context, INSTRUCTION_SIZE);
    };
    let value = pop_any(frame)?;
    let object_ref = if let Some(reference) = pop_reference(frame)? {
        reference
    } else {
        throw_null_pointer_exception!(frame, context, INSTRUCTION_SIZE);
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

#[inline]
fn generic_static_field_instruction<F>(context: JvmContext, field_fn: F) -> JvmResult<()>
where
    F: FnOnce(&mut StaticFieldInfo, &mut JvmStackFrame) -> JvmResult<()>,
{
    const INSTRUCTION_SIZE: usize = 3;
    let frame = context.current_thread.top_frame();
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
        initialise_class_and_rewind!(frame, context, &class, INSTRUCTION_SIZE);
    }

    let (class, index) = find_static_field_class_with_index(class, field_name)?;
    let mut state = class.state.borrow_mut();
    let fields = state
        .static_fields
        .as_mut()
        .expect("Static fields not initialised");

    let class_file_index = fields[index].field_class_file_index;
    validate_access!(
        current_class,
        class,
        class.class_file.fields[class_file_index].access_flags,
        frame,
        context,
        3
    );

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
        class_name: class.class_file.get_class_name().to_owned(),
        field_name: name.to_owned(),
    }
    .bx())
}

enum AccessObjectFieldResult {
    HasAccess {
        field_index: usize,
        field_descriptor: DescriptorType,
    },
    NoAccess,
}

fn access_object_field(
    unvalidated_cp_index: u16,
    frame: &mut JvmStackFrame,
    class_loader: &mut ClassLoader,
) -> JvmResult<AccessObjectFieldResult> {
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

        return Ok(AccessObjectFieldResult::HasAccess {
            field_index: info.field_index,
            field_descriptor,
        });
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
    let has_access = check_has_access(
        &current_class,
        &declared_class,
        field_class.class_file.fields[declared_class_file_field_index].access_flags,
    );
    if !has_access {
        return Ok(AccessObjectFieldResult::NoAccess);
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

    Ok(AccessObjectFieldResult::HasAccess {
        field_index,
        field_descriptor,
    })
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
        class_name: declared_class.class_file.get_class_name().to_owned(),
        field_name: field_name.to_owned(),
    }
    .bx())
}
