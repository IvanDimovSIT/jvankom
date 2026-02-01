use std::{error::Error, fmt::Display};

use crate::{
    bytecode::{ILOAD, IRETURN, RETURN},
    class_file::{Attribute, Bytecode, ClassFile, Method},
    class_parser::UnverifiedClassFile,
};

const RETURN_INSTRUCTIONS: [u8; 2] = [RETURN, IRETURN];
/// instructions which load a local, based on the bytecode
const LOAD_N_INSTRUCTIONS: [u8; 1] = [ILOAD];

#[derive(Debug, Clone)]
pub enum VerifierError {
    MissingMethodDescriptor,
    MissingReturnFromMethod,
    InvalidLoadInstruction,
}
impl Display for VerifierError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let desc = match self {
            VerifierError::MissingMethodDescriptor => "Missing method descriptor",
            VerifierError::MissingReturnFromMethod => "Missing return instruction from method",
            VerifierError::InvalidLoadInstruction => "Invalid load instruction",
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

pub fn verify_class_file(
    unverified_class_file: UnverifiedClassFile,
) -> Result<ClassFile, VerifierError> {
    let class = unverified_class_file.mark_verified();
    verify_returns(&class)?;
    verify_loads(&class)?;
    Ok(class)
}

fn verify_returns(class: &ClassFile) -> Result<(), VerifierError> {
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

fn verify_loads(class: &ClassFile) -> Result<(), VerifierError> {
    for method in &class.methods {
        for atr in &method.attributes {
            match atr {
                Attribute::Code(bytecode) => {
                    verify_load_bytecode(bytecode)?;
                }
                _ => continue,
            }
        }
    }

    Ok(())
}

fn verify_load_bytecode(bytecode: &Bytecode) -> Result<(), VerifierError> {
    let bytecode_len = bytecode.code.len();
    if bytecode_len < 2 {
        return Ok(());
    }
    if LOAD_N_INSTRUCTIONS.contains(&bytecode.code[bytecode_len - 2]) {
        return Err(VerifierError::InvalidLoadInstruction);
    }
    for codes in bytecode.code.windows(2) {
        if !LOAD_N_INSTRUCTIONS.contains(&codes[0]) {
            continue;
        }

        let load_address = codes[1];
        if load_address as u16 >= bytecode.max_locals {
            return Err(VerifierError::InvalidLoadInstruction);
        }
    }

    Ok(())
}

fn get_descriptor<'a>(class: &'a ClassFile, method: &'a Method) -> Result<&'a str, VerifierError> {
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
