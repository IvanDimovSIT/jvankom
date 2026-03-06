use std::{error::Error, fmt::Display, num::NonZeroUsize};

use crate::{
    bytecode::{ALOAD, ASTORE, ILOAD, IRETURN, ISTORE, RETURN},
    class_file::{Attribute, Bytecode, ClassFile, ConstantValue, Method},
    class_parser::UnverifiedClassFile,
};

const RETURN_INSTRUCTIONS: [u8; 2] = [RETURN, IRETURN];
/// instructions which load or store a local, based on the bytecode
const LOAD_STORE_N_INSTRUCTIONS: [u8; 4] = [ILOAD, ISTORE, ALOAD, ASTORE];

pub type VerifierResult<T> = Result<T, VerifierError>;

#[derive(Debug, Clone)]
pub enum VerifierError {
    MissingMethodDescriptor,
    MissingReturnFromMethod,
    InvalidIndexingInstruction,
    InvalidNameConstantIndex,
    InvalidNameAndTypeIndex,
    InvalidUTF8Index,
    InvalidClassIndex,
}
impl Display for VerifierError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let desc = match self {
            VerifierError::MissingMethodDescriptor => "Missing method descriptor",
            VerifierError::MissingReturnFromMethod => "Missing return instruction from method",
            VerifierError::InvalidIndexingInstruction => "Invalid load instruction",
            VerifierError::InvalidNameConstantIndex => "Invalid name constant index",
            VerifierError::InvalidNameAndTypeIndex => "Invalid name and type index",
            VerifierError::InvalidUTF8Index => "Invalid UTF8 index",
            VerifierError::InvalidClassIndex => "Invalid class index",
        };

        f.write_str(desc)
    }
}
impl Error for VerifierError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}

pub fn verify_class_file(unverified_class_file: UnverifiedClassFile) -> VerifierResult<ClassFile> {
    let class = unverified_class_file.mark_verified();

    // TODO: Fix verification!
    // verify_returns(&class)?;
    // verify_load_and_stores(&class)?;
    verify_constant_pool(&class)?;
    Ok(class)
}

fn verify_returns(class: &ClassFile) -> VerifierResult<()> {
    for method in &class.methods {
        for atr in &method.attributes {
            let _descriptor = get_descriptor(class, method)?;

            let has_return = match atr {
                Attribute::Code(bytecode) => bytecode
                    .code
                    .last()
                    .map(|c| RETURN_INSTRUCTIONS.contains(c))
                    .unwrap_or(false),
                _ => {
                    continue;
                }
            };

            return if has_return {
                Ok(())
            } else {
                Err(VerifierError::MissingReturnFromMethod)
            };
        }
    }

    Ok(())
}

fn verify_load_and_stores(class: &ClassFile) -> VerifierResult<()> {
    for method in &class.methods {
        for atr in &method.attributes {
            match atr {
                Attribute::Code(bytecode) => {
                    verify_load_store_bytecode(bytecode)?;
                }
                _ => continue,
            }
        }
    }

    Ok(())
}

fn verify_load_store_bytecode(bytecode: &Bytecode) -> VerifierResult<()> {
    let bytecode_len = bytecode.code.len();
    if bytecode_len < 2 {
        return Ok(());
    }
    if LOAD_STORE_N_INSTRUCTIONS.contains(&bytecode.code[bytecode_len - 2]) {
        return Err(VerifierError::InvalidIndexingInstruction);
    }
    for codes in bytecode.code.windows(2) {
        if !LOAD_STORE_N_INSTRUCTIONS.contains(&codes[0]) {
            continue;
        }

        let address = codes[1];
        if address as u16 >= bytecode.max_locals {
            return Err(VerifierError::InvalidIndexingInstruction);
        }
    }

    Ok(())
}

fn verify_constant_pool(class: &ClassFile) -> VerifierResult<()> {
    for const_value in class.constant_pool.get_all_constants() {
        match *const_value {
            ConstantValue::MethodRef {
                class_index,
                name_and_type_index,
            } => verify_method_ref_constant(class, class_index, name_and_type_index)?,
            ConstantValue::Class { name_index } => verify_utf8_constant(class, name_index)?,
            ConstantValue::String { utf8_index } => verify_utf8_constant(class, utf8_index)?,
            ConstantValue::FieldRef {
                class_index,
                name_and_type_index,
            } => verify_field_ref_constant(class, class_index, name_and_type_index)?,
            ConstantValue::InterfaceMethodRef {
                class_index,
                name_and_type_index,
            } => verify_method_ref_constant(class, class_index, name_and_type_index)?,
            _ => {}
        }
    }

    Ok(())
}

fn verify_field_ref_constant(
    class: &ClassFile,
    class_index: NonZeroUsize,
    name_and_type_index: NonZeroUsize,
) -> VerifierResult<()> {
    verify_class_index(class, class_index)?;
    verify_name_and_type_index(class, name_and_type_index)?;

    Ok(())
}

fn verify_name_and_type_index(
    class: &ClassFile,
    name_and_type_index: NonZeroUsize,
) -> VerifierResult<()> {
    if class
        .constant_pool
        .get_name_and_type(name_and_type_index)
        .is_some()
    {
        Ok(())
    } else {
        Err(VerifierError::InvalidNameAndTypeIndex)
    }
}

fn verify_class_index(class: &ClassFile, class_index: NonZeroUsize) -> VerifierResult<()> {
    if class.constant_pool.get_class_name(class_index).is_some() {
        Ok(())
    } else {
        Err(VerifierError::InvalidClassIndex)
    }
}

fn verify_utf8_constant(class: &ClassFile, index: NonZeroUsize) -> VerifierResult<()> {
    if class.constant_pool.get_utf8(index).is_some() {
        Ok(())
    } else {
        Err(VerifierError::InvalidUTF8Index)
    }
}

fn verify_method_ref_constant(
    class: &ClassFile,
    class_index: NonZeroUsize,
    name_and_type_index: NonZeroUsize,
) -> VerifierResult<()> {
    verify_class_index(class, class_index)?;
    verify_name_and_type_index(class, name_and_type_index)?;

    Ok(())
}

fn get_descriptor<'a>(class: &'a ClassFile, method: &'a Method) -> VerifierResult<&'a str> {
    if let Some(desc) = class.constant_pool.get_utf8(method.descriptor_index) {
        Ok(desc)
    } else {
        Err(VerifierError::MissingMethodDescriptor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::class_parser;

    const TEST_CLASS_FILE_PATH: &str = "test_classes/Test.class";

    #[test]
    fn test_verify_ok() {
        let class = class_parser::parse(TEST_CLASS_FILE_PATH).unwrap();
        verify_class_file(class).unwrap();
    }
}
