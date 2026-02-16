use std::num::NonZeroUsize;

use crate::{
    bytecode::{pop_int, pop_long, pop_reference},
    jvm_model::{DescriptorType, JvmError, JvmResult, JvmStackFrame, JvmValue},
};

pub fn parse_descriptor(method_descriptor: &str) -> JvmResult<Vec<DescriptorType>> {
    let param_list_start = if let Some(start) = method_descriptor.find('(') {
        start + 1
    } else {
        return Err(JvmError::InvalidMethodDescriptor(method_descriptor.to_owned()).bx());
    };
    let param_list_end = if let Some(end) = method_descriptor.find(')') {
        end
    } else {
        return Err(JvmError::InvalidMethodDescriptor(method_descriptor.to_owned()).bx());
    };
    let params_desc = &method_descriptor[param_list_start..param_list_end];

    let mut types = Vec::with_capacity(4);
    let mut in_ref = false;
    for param_desc in params_desc.chars() {
        if in_ref {
            if param_desc == ';' {
                in_ref = false;
            }
        } else {
            match param_desc {
                'I' => types.push(DescriptorType::Integer),
                'J' => types.push(DescriptorType::Long),
                'F' => types.push(DescriptorType::Float),
                'D' => types.push(DescriptorType::Double),
                'B' => types.push(DescriptorType::Byte),
                'C' => types.push(DescriptorType::Character),
                'S' => types.push(DescriptorType::Short),
                'Z' => types.push(DescriptorType::Boolean),
                _ => {
                    types.push(DescriptorType::Reference);
                    debug_assert!(['[', 'L'].contains(&param_desc));
                    debug_assert_ne!(';', param_desc);
                    in_ref = true;
                }
            };
        }
    }

    types.reverse();

    Ok(types)
}

/// types need to be in pop order (reversed)
pub fn pop_params_for_virtual(
    object_ref: NonZeroUsize,
    types: &[DescriptorType],
    frame: &mut JvmStackFrame,
) -> JvmResult<Vec<JvmValue>> {
    let mut params = Vec::with_capacity(types.len() + 1);
    params.push(JvmValue::Reference(Some(object_ref)));
    params.extend(pop_params(types, frame)?);

    Ok(params)
}

/// types need to be in pop order (reversed)
pub fn pop_params(types: &[DescriptorType], frame: &mut JvmStackFrame) -> JvmResult<Vec<JvmValue>> {
    let mut params = Vec::with_capacity(4);

    for t in types {
        match *t {
            DescriptorType::Integer => params.insert(0, JvmValue::Int(pop_int(frame)?)),
            DescriptorType::Long => {
                params.insert(0, JvmValue::Unusable);
                params.insert(0, JvmValue::Long(pop_long(frame)?));
            }
            DescriptorType::Reference => {
                params.insert(0, JvmValue::Reference(pop_reference(frame)?));
            }
            _ => unimplemented!(),
        };
    }

    Ok(params)
}
