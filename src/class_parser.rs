use std::{error::Error, fmt::Display};

use crate::class_file::{ClassFile, ConstantValue};

const CLASS_FILE_MAGIC: u32 = 0xCAFEBABE;
const UTF8_TAG: u8 = 1;
const INTEGER_TAG: u8 = 3;
const FLOAT_TAG: u8 = 4;
const LONG_TAG: u8 = 5;
const DOUBLE_TAG: u8 = 6;
const CLASS_TAG: u8 = 7;
const STRING_TAG: u8 = 8;
const FIELDREF_TAG: u8 = 9;
const METHODREF_TAG: u8 = 10;
const INTERFACE_METHODREF_TAG: u8 = 11;
const NAME_AND_TYPE_TAG: u8 = 12;
const METHOD_HANDLE_TAG: u8 = 15;
const METHOD_TYPE_TAG: u8 = 16;
const INVOKE_DYNAMIC_TAG: u8 = 18;

pub fn parse(class_file_path: &str) -> Result<ClassFile, ClassParserError> {
    let bytes_result = std::fs::read(class_file_path);
    if let Ok(bytes) = bytes_result {
        parse_from_bytes(bytes)
    } else {
        Err(ClassParserError::ErrorReadingFile(
            bytes_result.unwrap_err().to_string(),
        ))
    }
}

pub fn parse_from_bytes(bytes: Vec<u8>) -> Result<ClassFile, ClassParserError> {
    if bytes.is_empty() {
        return Err(ClassParserError::EmptyFile);
    }
    let parser = ClassParser::new(bytes);
    parser.parse_class_file()
}

#[derive(Debug, Clone)]
pub enum ClassParserError {
    EmptyFile,
    ErrorReadingFile(String),
    UnexpectedEndOfFile,
    InvalidMagicNumber,
    InvalidConstantsPoolSize,
    InvalidTag,
    InvalidUtf8String(String),
}
impl Display for ClassParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let description = match self {
            Self::EmptyFile => "File is empty",
            Self::ErrorReadingFile(desc) => desc,
            Self::UnexpectedEndOfFile => "Unexpected end of file",
            Self::InvalidMagicNumber => "Invalid magic number",
            Self::InvalidConstantsPoolSize => "Invalid constants pool size",
            Self::InvalidTag => "Invalid tag",
            Self::InvalidUtf8String(desc) => desc,
        };

        f.write_str(&description)
    }
}
impl Error for ClassParserError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}

struct ClassParser {
    bytes: Vec<u8>,
    index: usize,
}
impl ClassParser {
    fn new(bytes: Vec<u8>) -> Self {
        Self { bytes, index: 0 }
    }

    fn parse_class_file(mut self) -> Result<ClassFile, ClassParserError> {
        self.validate_magic()?;
        let (_major, _minor) = self.parse_versions()?;
        let constant_pool = self.parse_constant_pool()?;

        todo!()
    }

    fn parse_constant_pool(&mut self) -> Result<Vec<ConstantValue>, ClassParserError> {
        let constants_size = self.parse_u16()? as usize;
        if constants_size == 0 {
            return Err(ClassParserError::InvalidConstantsPoolSize);
        }

        let mut constants_pool = Vec::with_capacity(constants_size);
        constants_pool.push(ConstantValue::Unusable);
        let mut constants_index = 1;
        while constants_index < constants_size {
            let tag = self.parse_u8()?;
            constants_pool.push(self.parse_constant_value(tag)?);
            if tag == DOUBLE_TAG || tag == LONG_TAG {
                constants_pool.push(ConstantValue::Unusable);
                constants_index += 2;
            } else {
                constants_index += 1;
            }
        }
        assert_eq!(constants_size, constants_pool.len());

        Ok(constants_pool)
    }

    fn parse_constant_value(&mut self, tag: u8) -> Result<ConstantValue, ClassParserError> {
        let constant_value = match tag {
            UTF8_TAG => self.parse_utf8_constant(),
            INTEGER_TAG => todo!(),
            FLOAT_TAG => todo!(),
            LONG_TAG => todo!(),
            DOUBLE_TAG => todo!(),
            CLASS_TAG => todo!(),
            STRING_TAG => todo!(),
            FIELDREF_TAG => todo!(),
            METHODREF_TAG => todo!(),
            INTERFACE_METHODREF_TAG => todo!(),
            NAME_AND_TYPE_TAG => todo!(),
            METHOD_HANDLE_TAG => todo!(),
            METHOD_TYPE_TAG => todo!(),
            INVOKE_DYNAMIC_TAG => todo!(),
            _ => Err(ClassParserError::InvalidTag),
        }?;

        Ok(constant_value)
    }

    fn parse_utf8_constant(&mut self) -> Result<ConstantValue, ClassParserError> {
        let size = self.parse_u16()? as usize;
        let bytes = self.parse_byte_array(size)?;
        let parsed_string = String::from_utf8(bytes);

        let string = if let Ok(ok_string) = parsed_string {
            ok_string
        } else {
            return Err(ClassParserError::InvalidUtf8String(
                parsed_string.unwrap_err().to_string(),
            ));
        };

        Ok(ConstantValue::Utf8(string))
    }

    fn parse_versions(&mut self) -> Result<(u16, u16), ClassParserError> {
        Ok((self.parse_u16()?, self.parse_u16()?))
    }

    fn validate_magic(&mut self) -> Result<(), ClassParserError> {
        let magic = self.parse_u32()?;
        if magic == CLASS_FILE_MAGIC {
            Ok(())
        } else {
            Err(ClassParserError::InvalidMagicNumber)
        }
    }

    fn parse_u8(&mut self) -> Result<u8, ClassParserError> {
        if self.check_left(1) {
            Ok(self.read_u8())
        } else {
            Err(ClassParserError::UnexpectedEndOfFile)
        }
    }

    fn parse_u16(&mut self) -> Result<u16, ClassParserError> {
        if self.check_left(2) {
            Ok(self.read_u16())
        } else {
            Err(ClassParserError::UnexpectedEndOfFile)
        }
    }

    fn parse_u32(&mut self) -> Result<u32, ClassParserError> {
        if self.check_left(4) {
            Ok(self.read_u32())
        } else {
            Err(ClassParserError::UnexpectedEndOfFile)
        }
    }

    fn parse_byte_array(&mut self, size: usize) -> Result<Vec<u8>, ClassParserError> {
        if self.check_left(size) {
            let end_index = self.index + size;
            let bytes = self.bytes[self.index..end_index].to_vec();
            self.index = end_index;

            Ok(bytes)
        } else {
            Err(ClassParserError::UnexpectedEndOfFile)
        }
    }

    fn skip(&mut self, bytes_to_skip: usize) -> Result<(), ClassParserError> {
        self.index += bytes_to_skip;
        if self.index > self.bytes.len() {
            Err(ClassParserError::UnexpectedEndOfFile)
        } else {
            Ok(())
        }
    }

    fn read_u8(&mut self) -> u8 {
        let value = self.bytes[self.index];
        self.index += 1;
        value
    }

    fn read_u16(&mut self) -> u16 {
        ((self.read_u8() as u16) << 8) | self.read_u8() as u16
    }

    fn read_u32(&mut self) -> u32 {
        ((self.read_u16() as u32) << 16) | self.read_u16() as u32
    }

    fn check_left(&self, bytes_to_check: usize) -> bool {
        self.index + bytes_to_check <= self.bytes.len()
    }
}
