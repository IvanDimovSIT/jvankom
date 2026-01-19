use std::{error::Error, fmt::Display, num::NonZeroUsize};

use crate::class_file::{Attribute, ClassFile, ConstantValue, Field};

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

const CODE_ATTRIBUTE_NAME: &str = "Code";
const CONSTATNT_VALUE_ATTRIBUTE_NAME: &str = "ConstantValue";
const SOURCE_FILE_ATTRIBUTE_NAME: &str = "SourceFile";

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
    InvalidReferenceKind,
    ExpectedUtf8,
    InvalidAttributeLength,
    InvalidConstantPoolIndex,
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
            Self::InvalidReferenceKind => "Invalid reference kind",
            Self::ExpectedUtf8 => "Expected UTF8 constant value",
            Self::InvalidAttributeLength => "Invalid attribute length",
            Self::InvalidConstantPoolIndex => "Invalid constant pool index",
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
        let access_flags = self.parse_u16()?;
        let this_class = self.parse_u16()? as usize;
        let super_class = self.parse_super_class()?;
        let interfaces = self.parse_interfaces()?;
        let fields = self.parse_fields(&constant_pool)?;
        let methods = todo!();
        let attributes = self.parse_attributes(&constant_pool)?;

        let class_file = ClassFile {
            class_index: this_class,
            super_class_index: super_class,
            interfaces,
            constant_pool,
            methods,
            fields,
            access_flags,
            attributes,
        };

        Ok(class_file)
    }

    fn parse_fields(
        &mut self,
        constant_pool: &[ConstantValue],
    ) -> Result<Vec<Field>, ClassParserError> {
        let fields_count = self.parse_u16()? as usize;
        let mut fields = Vec::with_capacity(fields_count);
        for _ in 0..fields_count {
            let field = self.parse_field(constant_pool)?;
            fields.push(field);
        }

        Ok(fields)
    }

    fn parse_field(&mut self, constant_pool: &[ConstantValue]) -> Result<Field, ClassParserError> {
        let access_flags = self.parse_u16()?;
        let name_index = self.parse_u16()? as usize;
        let descriptor_index = self.parse_u16()? as usize;
        let attributes = self.parse_attributes(constant_pool)?;

        let field = Field {
            name_index,
            descriptor_index,
            access_flags,
            attributes,
        };

        Ok(field)
    }

    fn parse_attribute(&mut self, attribute_name: &str) -> Result<Attribute, ClassParserError> {
        let attribute_length = self.parse_u32()? as usize;
        let attribute = match attribute_name {
            CODE_ATTRIBUTE_NAME => {
                todo!()
            }
            CONSTATNT_VALUE_ATTRIBUTE_NAME => {
                Self::expect_attribute_length(2, attribute_length)?;
                let value_index = self.parse_u16()? as usize;
                Attribute::ConstantValue { value_index }
            }
            SOURCE_FILE_ATTRIBUTE_NAME => {
                Self::expect_attribute_length(2, attribute_length)?;
                let sourcefile_index = self.parse_u16()? as usize;
                Attribute::SourceFile { sourcefile_index }
            }
            _ => {
                let info = self.parse_byte_array(attribute_length)?;
                Attribute::Unknown {
                    name: attribute_name.to_owned(),
                    info,
                }
            }
        };

        Ok(attribute)
    }

    fn expect_attribute_length(expected: usize, actual: usize) -> Result<(), ClassParserError> {
        if expected == actual {
            Ok(())
        } else {
            Err(ClassParserError::InvalidAttributeLength)
        }
    }

    fn parse_attributes(
        &mut self,
        constant_pool: &[ConstantValue],
    ) -> Result<Vec<Attribute>, ClassParserError> {
        let attributes_count = self.parse_u16()? as usize;
        let mut attributes = Vec::with_capacity(attributes_count);
        for _ in 0..attributes_count {
            let attribute_name_index = self.parse_u16()? as usize;
            let attribute_name = Self::index_utf8(constant_pool, attribute_name_index)?;
            let attribute = self.parse_attribute(attribute_name)?;
            attributes.push(attribute);
        }

        Ok(attributes)
    }

    fn index_utf8(constant_pool: &[ConstantValue], index: usize) -> Result<&str, ClassParserError> {
        let constant_value = Self::index_constant_pool(constant_pool, index)?;
        match constant_value {
            ConstantValue::Utf8(s) => Ok(s),
            _ => Err(ClassParserError::ExpectedUtf8),
        }
    }

    fn index_constant_pool(
        constant_pool: &[ConstantValue],
        index: usize,
    ) -> Result<&ConstantValue, ClassParserError> {
        if index == 0 || index >= constant_pool.len() {
            Err(ClassParserError::InvalidConstantPoolIndex)
        } else {
            Ok(&constant_pool[index])
        }
    }

    fn parse_interfaces(&mut self) -> Result<Vec<usize>, ClassParserError> {
        let interfaces_size = self.parse_u16()? as usize;
        let mut intefaces = Vec::with_capacity(interfaces_size);
        for _ in 0..interfaces_size {
            let interface_index = self.parse_u16()? as usize;
            intefaces.push(interface_index);
        }

        Ok(intefaces)
    }

    fn parse_super_class(&mut self) -> Result<Option<NonZeroUsize>, ClassParserError> {
        let index = self.parse_u16()?;
        Ok(NonZeroUsize::new(index as usize))
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
            INTEGER_TAG => self.parse_integer(),
            FLOAT_TAG => self.parse_float(),
            LONG_TAG => self.parse_long(),
            DOUBLE_TAG => self.parse_double(),
            CLASS_TAG => self.parse_class_const(),
            STRING_TAG => self.parse_string(),
            FIELDREF_TAG => self.parse_field_ref(),
            METHODREF_TAG => self.parse_method_ref(),
            INTERFACE_METHODREF_TAG => self.parse_interface_method_ref(),
            NAME_AND_TYPE_TAG => self.parse_name_and_type(),
            METHOD_HANDLE_TAG => self.parse_method_handle(),
            METHOD_TYPE_TAG => self.parse_method_type(),
            INVOKE_DYNAMIC_TAG => self.parse_invoke_dynamic(),
            _ => Err(ClassParserError::InvalidTag),
        }?;

        Ok(constant_value)
    }

    fn parse_invoke_dynamic(&mut self) -> Result<ConstantValue, ClassParserError> {
        let bootstrap_method_attr_index = self.parse_u16()?;
        let name_and_type_index = self.parse_u16()?;
        Ok(ConstantValue::InvokeDynamic {
            bootstrap_method_attr_index: bootstrap_method_attr_index as usize,
            name_and_type_index: name_and_type_index as usize,
        })
    }

    fn parse_method_type(&mut self) -> Result<ConstantValue, ClassParserError> {
        let index = self.parse_u16()?;
        Ok(ConstantValue::MethodType {
            descriptor_index: index as usize,
        })
    }

    fn parse_method_handle(&mut self) -> Result<ConstantValue, ClassParserError> {
        let reference_kind = self.parse_u8()?;
        if !(1..=9).contains(&reference_kind) {
            return Err(ClassParserError::InvalidReferenceKind);
        }

        let reference_index = self.parse_u16()?;
        Ok(ConstantValue::MethodHandle {
            reference_kind,
            reference_index: reference_index as usize,
        })
    }

    fn parse_name_and_type(&mut self) -> Result<ConstantValue, ClassParserError> {
        let name_index = self.parse_u16()?;
        let descriptor_index = self.parse_u16()?;
        Ok(ConstantValue::NameAndType {
            name_index: name_index as usize,
            descriptor_index: descriptor_index as usize,
        })
    }

    fn parse_interface_method_ref(&mut self) -> Result<ConstantValue, ClassParserError> {
        let class_index = self.parse_u16()?;
        let name_and_type_index = self.parse_u16()?;

        Ok(ConstantValue::InterfaceMethodRef {
            class_index: class_index as usize,
            name_and_type_index: name_and_type_index as usize,
        })
    }

    fn parse_method_ref(&mut self) -> Result<ConstantValue, ClassParserError> {
        let class_index = self.parse_u16()?;
        let name_and_type_index = self.parse_u16()?;

        Ok(ConstantValue::MethodRef {
            class_index: class_index as usize,
            name_and_type_index: name_and_type_index as usize,
        })
    }

    fn parse_field_ref(&mut self) -> Result<ConstantValue, ClassParserError> {
        let class_index = self.parse_u16()?;
        let name_and_type_index = self.parse_u16()?;
        Ok(ConstantValue::FieldRef {
            class_index: class_index as usize,
            name_and_type_index: name_and_type_index as usize,
        })
    }

    fn parse_string(&mut self) -> Result<ConstantValue, ClassParserError> {
        let index = self.parse_u16()?;
        Ok(ConstantValue::String {
            utf8_index: index as usize,
        })
    }

    fn parse_class_const(&mut self) -> Result<ConstantValue, ClassParserError> {
        let index = self.parse_u16()?;
        Ok(ConstantValue::Class {
            name_index: index as usize,
        })
    }

    fn parse_integer(&mut self) -> Result<ConstantValue, ClassParserError> {
        let int_bytes = self.parse_u32()?;
        Ok(ConstantValue::Int(int_bytes as i32))
    }

    fn parse_float(&mut self) -> Result<ConstantValue, ClassParserError> {
        let float_bytes = self.parse_u32()?;
        Ok(ConstantValue::Float(f32::from_bits(float_bytes)))
    }

    fn parse_long(&mut self) -> Result<ConstantValue, ClassParserError> {
        let long_bytes = self.parse_u64()?;
        Ok(ConstantValue::Long(long_bytes as i64))
    }

    fn parse_double(&mut self) -> Result<ConstantValue, ClassParserError> {
        let double_bytes = self.parse_u64()?;
        Ok(ConstantValue::Double(f64::from_bits(double_bytes)))
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

    fn parse_u64(&mut self) -> Result<u64, ClassParserError> {
        if self.check_left(8) {
            Ok(self.read_u64())
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

    fn read_u8(&mut self) -> u8 {
        let value = self.bytes[self.index];
        self.index += 1;
        value
    }

    fn read_u16(&mut self) -> u16 {
        let high_bits = self.read_u8() as u16;
        let low_bits = self.read_u8() as u16;
        (high_bits << 8) | low_bits
    }

    fn read_u32(&mut self) -> u32 {
        let high_bits = self.read_u16() as u32;
        let low_bits = self.read_u16() as u32;
        (high_bits << 16) | low_bits
    }

    fn read_u64(&mut self) -> u64 {
        let high_bits = self.read_u32() as u64;
        let low_bits = self.read_u32() as u64;
        (high_bits << 32) | low_bits
    }

    fn check_left(&self, bytes_to_check: usize) -> bool {
        self.index + bytes_to_check <= self.bytes.len()
    }
}
