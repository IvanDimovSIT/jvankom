use crate::{
    bytecode::{pop_int, pop_long, pop_reference},
    jvm_model::{JvmError, JvmResult, JvmStackFrame, JvmValue},
};

pub fn prepare_method_parameters(
    frame: &mut JvmStackFrame,
    method_descriptor: &str,
) -> JvmResult<Vec<JvmValue>> {
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

    pop_params(params_desc, frame)
}

fn pop_params(params_desc: &str, frame: &mut JvmStackFrame) -> JvmResult<Vec<JvmValue>> {
    let mut params = Vec::with_capacity(4);
    let mut types = Vec::with_capacity(4);

    let mut in_ref = false;
    for param_desc in params_desc.chars() {
        if in_ref {
            if param_desc == ';' {
                in_ref = false;
            }
        } else {
            match param_desc {
                'I' => types.push('I'),
                'J' => types.push('J'),
                _ => {
                    types.push('L');
                    debug_assert!(['[', 'L'].contains(&param_desc));
                    debug_assert_ne!(';', param_desc);
                    in_ref = true;
                }
            };
        }
    }

    for t in types.iter().rev() {
        match t {
            'I' => params.insert(0, JvmValue::Int(pop_int(frame)?)),
            'J' => {
                params.insert(0, JvmValue::Unusable);
                params.insert(0, JvmValue::Long(pop_long(frame)?));
            }
            _ => {
                params.insert(0, JvmValue::Reference(pop_reference(frame)?));
            }
        };
    }

    Ok(params)
}
