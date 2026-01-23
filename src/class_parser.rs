use std::{error::Error, fmt::Display, num::NonZeroUsize};

use crate::class_file::{
    Attribute, Bytecode, ClassAccessFlags, ClassFile, ConstantPool, ConstantValue,
    ExceptionTableEntry, Field, FieldAccessFlags, Method, MethodAccessFlags,
};

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
const CONSTANT_VALUE_ATTRIBUTE_NAME: &str = "ConstantValue";
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
    ExpectedEndOfFile,
    InvalidMagicNumber,
    InvalidConstantsPoolSize,
    InvalidTag,
    InvalidUtf8String(String),
    InvalidReferenceKind,
    ExpectedUtf8,
    InvalidAttributeLength,
    InvalidConstantPoolIndex,
    InvalidFieldAccessFlags,
    InvalidMethodAccessFlags,
    InvalidClassAccessFlags,
}
impl Display for ClassParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let description = match self {
            Self::EmptyFile => "File is empty",
            Self::ErrorReadingFile(desc) => desc,
            Self::UnexpectedEndOfFile => "Unexpected end of file",
            Self::ExpectedEndOfFile => "Expecting end of file - extra bytes present",
            Self::InvalidMagicNumber => "Invalid magic number",
            Self::InvalidConstantsPoolSize => "Invalid constants pool size",
            Self::InvalidTag => "Invalid constant pool tag",
            Self::InvalidUtf8String(desc) => desc,
            Self::InvalidReferenceKind => "Invalid reference kind",
            Self::ExpectedUtf8 => "Expected UTF8 constant value",
            Self::InvalidAttributeLength => "Invalid attribute length",
            Self::InvalidConstantPoolIndex => "Invalid constant pool index",
            Self::InvalidFieldAccessFlags => "Invalid field access flags",
            Self::InvalidMethodAccessFlags => "Invalid method access flags",
            Self::InvalidClassAccessFlags => "Invalid class access flags",
        };

        f.write_str(description)
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
        let access_flags = self.parse_class_access_flags()?;
        let this_class = self.parse_index(constant_pool.len())?;
        let super_class = self.parse_super_class(constant_pool.len())?;
        let interfaces = self.parse_interfaces(constant_pool.len())?;
        let fields = self.parse_fields(&constant_pool)?;
        let methods = self.parse_methods(&constant_pool)?;
        let attributes = self.parse_attributes(&constant_pool)?;

        if self.index != self.bytes.len() {
            return Err(ClassParserError::ExpectedEndOfFile);
        }

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

    fn parse_class_access_flags(&mut self) -> Result<ClassAccessFlags, ClassParserError> {
        let raw_flags = self.parse_u16()?;

        if let Some(access_flags) = ClassAccessFlags::new(raw_flags) {
            Ok(access_flags)
        } else {
            Err(ClassParserError::InvalidClassAccessFlags)
        }
    }

    fn parse_methods(
        &mut self,
        constant_pool: &ConstantPool,
    ) -> Result<Vec<Method>, ClassParserError> {
        let methods_count = self.parse_u16()? as usize;
        let mut methods = Vec::with_capacity(methods_count);
        for _ in 0..methods_count {
            let method = self.parse_method(constant_pool)?;
            methods.push(method);
        }

        Ok(methods)
    }

    fn parse_method(&mut self, constant_pool: &ConstantPool) -> Result<Method, ClassParserError> {
        let access_flags = self.parse_method_access_flags()?;
        let name_index = self.parse_index(constant_pool.len())?;
        let descriptor_index = self.parse_index(constant_pool.len())?;
        let attributes = self.parse_attributes(constant_pool)?;

        let method = Method {
            name_index,
            descriptor_index,
            access_flags,
            attributes,
        };
        Ok(method)
    }

    fn parse_method_access_flags(&mut self) -> Result<MethodAccessFlags, ClassParserError> {
        let raw_flags = self.parse_u16()?;

        if let Some(access_flags) = MethodAccessFlags::new(raw_flags) {
            Ok(access_flags)
        } else {
            Err(ClassParserError::InvalidMethodAccessFlags)
        }
    }

    fn parse_fields(
        &mut self,
        constant_pool: &ConstantPool,
    ) -> Result<Vec<Field>, ClassParserError> {
        let fields_count = self.parse_u16()? as usize;
        let mut fields = Vec::with_capacity(fields_count);
        for _ in 0..fields_count {
            let field = self.parse_field(constant_pool)?;
            fields.push(field);
        }

        Ok(fields)
    }

    fn parse_field(&mut self, constant_pool: &ConstantPool) -> Result<Field, ClassParserError> {
        let access_flags = self.parse_field_access_flags()?;
        let name_index = self.parse_index(constant_pool.len())?;
        let descriptor_index = self.parse_index(constant_pool.len())?;
        let attributes = self.parse_attributes(constant_pool)?;

        let field = Field {
            name_index,
            descriptor_index,
            access_flags,
            attributes,
        };

        Ok(field)
    }

    fn parse_field_access_flags(&mut self) -> Result<FieldAccessFlags, ClassParserError> {
        let raw_flags = self.parse_u16()?;

        if let Some(access_flags) = FieldAccessFlags::new(raw_flags) {
            Ok(access_flags)
        } else {
            Err(ClassParserError::InvalidFieldAccessFlags)
        }
    }

    fn parse_attribute(
        &mut self,
        constant_pool: &ConstantPool,
        attribute_name_index: NonZeroUsize,
    ) -> Result<Attribute, ClassParserError> {
        let attribute_length = self.parse_u32()? as usize;
        let attribute_name = Self::index_utf8(constant_pool, attribute_name_index)?;
        let start_index = self.index;
        let attribute = match attribute_name {
            CODE_ATTRIBUTE_NAME => self.parse_bytecode_attribute(constant_pool)?,
            CONSTANT_VALUE_ATTRIBUTE_NAME => {
                let value_index = self.parse_index(constant_pool.len())?;
                Attribute::ConstantValue { value_index }
            }
            SOURCE_FILE_ATTRIBUTE_NAME => {
                let sourcefile_index = self.parse_index(constant_pool.len())?;
                Attribute::SourceFile { sourcefile_index }
            }
            _ => {
                let info = self.parse_byte_array(attribute_length)?;
                Attribute::Unknown {
                    name_index: attribute_name_index,
                    info,
                }
            }
        };
        let read_bytes = self.index - start_index;
        Self::expect_attribute_length(attribute_length, read_bytes)?;

        Ok(attribute)
    }

    fn parse_bytecode_attribute(
        &mut self,
        constant_pool: &ConstantPool,
    ) -> Result<Attribute, ClassParserError> {
        let max_stack = self.parse_u16()?;
        let max_locals = self.parse_u16()?;
        let code_length = self.parse_u32()? as usize;
        let code = self.parse_byte_array(code_length)?;
        let exception_table_length = self.parse_u16()? as usize;

        let mut exception_table = Vec::with_capacity(exception_table_length);
        for _ in 0..exception_table_length {
            let start_pc = self.parse_u16()?;
            let end_pc = self.parse_u16()?;
            let handler_pc = self.parse_u16()?;
            let catch_type = self.parse_u16()?;

            let exception_table_entry = ExceptionTableEntry {
                start_pc,
                end_pc,
                handler_pc,
                catch_type,
            };
            exception_table.push(exception_table_entry)
        }

        let attributes = self.parse_attributes(constant_pool)?;

        let bytecode = Bytecode {
            code,
            max_stack,
            max_locals,
            exception_table,
            attributes,
        };

        Ok(Attribute::Code(bytecode))
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
        constant_pool: &ConstantPool,
    ) -> Result<Vec<Attribute>, ClassParserError> {
        let attributes_count = self.parse_u16()? as usize;
        let mut attributes = Vec::with_capacity(attributes_count);
        for _ in 0..attributes_count {
            let attribute_name_index = self.parse_index(constant_pool.len())?;
            let attribute = self.parse_attribute(constant_pool, attribute_name_index)?;
            attributes.push(attribute);
        }

        Ok(attributes)
    }

    fn index_utf8(
        constant_pool: &ConstantPool,
        index: NonZeroUsize,
    ) -> Result<&str, ClassParserError> {
        let constant_value = constant_pool.get_utf8(index);
        if let Some(s) = constant_value {
            Ok(s)
        } else {
            Err(ClassParserError::ExpectedUtf8)
        }
    }

    fn parse_interfaces(
        &mut self,
        constant_pool_size: usize,
    ) -> Result<Vec<NonZeroUsize>, ClassParserError> {
        let interfaces_size = self.parse_u16()? as usize;
        let mut intefaces = Vec::with_capacity(interfaces_size);
        for _ in 0..interfaces_size {
            let interface_index = self.parse_index(constant_pool_size)?;
            intefaces.push(interface_index);
        }

        Ok(intefaces)
    }

    fn parse_super_class(
        &mut self,
        constant_pool_size: usize,
    ) -> Result<Option<NonZeroUsize>, ClassParserError> {
        let index = self.parse_u16()? as usize;
        if index >= constant_pool_size {
            return Err(ClassParserError::InvalidConstantPoolIndex);
        }

        Ok(NonZeroUsize::new(index))
    }

    fn parse_constant_pool(&mut self) -> Result<ConstantPool, ClassParserError> {
        let constants_size = self.parse_u16()? as usize;
        if constants_size == 0 {
            return Err(ClassParserError::InvalidConstantsPoolSize);
        }

        let mut constants_pool_values = Vec::with_capacity(constants_size);
        constants_pool_values.push(ConstantValue::Unusable);
        let mut constants_index = 1;
        while constants_index < constants_size {
            let tag = self.parse_u8()?;
            constants_pool_values.push(self.parse_constant_value(tag, constants_size)?);
            if tag == DOUBLE_TAG || tag == LONG_TAG {
                constants_pool_values.push(ConstantValue::Unusable);
                constants_index += 2;
            } else {
                constants_index += 1;
            }
        }
        assert_eq!(constants_size, constants_pool_values.len());
        let constant_pool = ConstantPool::new(constants_pool_values);

        Ok(constant_pool)
    }

    fn parse_constant_value(
        &mut self,
        tag: u8,
        constant_pool_size: usize,
    ) -> Result<ConstantValue, ClassParserError> {
        let constant_value = match tag {
            UTF8_TAG => self.parse_utf8_constant(),
            INTEGER_TAG => self.parse_integer(),
            FLOAT_TAG => self.parse_float(),
            LONG_TAG => self.parse_long(),
            DOUBLE_TAG => self.parse_double(),
            CLASS_TAG => self.parse_class_const(constant_pool_size),
            STRING_TAG => self.parse_string(constant_pool_size),
            FIELDREF_TAG => self.parse_field_ref(constant_pool_size),
            METHODREF_TAG => self.parse_method_ref(constant_pool_size),
            INTERFACE_METHODREF_TAG => self.parse_interface_method_ref(constant_pool_size),
            NAME_AND_TYPE_TAG => self.parse_name_and_type(constant_pool_size),
            METHOD_HANDLE_TAG => self.parse_method_handle(constant_pool_size),
            METHOD_TYPE_TAG => self.parse_method_type(constant_pool_size),
            INVOKE_DYNAMIC_TAG => self.parse_invoke_dynamic(constant_pool_size),
            _ => Err(ClassParserError::InvalidTag),
        }?;

        Ok(constant_value)
    }

    fn parse_invoke_dynamic(
        &mut self,
        constant_pool_size: usize,
    ) -> Result<ConstantValue, ClassParserError> {
        let bootstrap_method_attr_index = self.parse_index(constant_pool_size)?;
        let name_and_type_index = self.parse_index(constant_pool_size)?;
        Ok(ConstantValue::InvokeDynamic {
            bootstrap_method_attr_index,
            name_and_type_index,
        })
    }

    fn parse_method_type(
        &mut self,
        constant_pool_size: usize,
    ) -> Result<ConstantValue, ClassParserError> {
        let descriptor_index = self.parse_index(constant_pool_size)?;
        Ok(ConstantValue::MethodType { descriptor_index })
    }

    fn parse_method_handle(
        &mut self,
        constant_pool_size: usize,
    ) -> Result<ConstantValue, ClassParserError> {
        let reference_kind = self.parse_u8()?;
        if !(1..=9).contains(&reference_kind) {
            return Err(ClassParserError::InvalidReferenceKind);
        }

        let reference_index = self.parse_index(constant_pool_size)?;
        Ok(ConstantValue::MethodHandle {
            reference_kind,
            reference_index,
        })
    }

    fn parse_name_and_type(
        &mut self,
        constant_pool_size: usize,
    ) -> Result<ConstantValue, ClassParserError> {
        let name_index = self.parse_index(constant_pool_size)?;
        let descriptor_index = self.parse_index(constant_pool_size)?;
        Ok(ConstantValue::NameAndType {
            name_index,
            descriptor_index,
        })
    }

    fn parse_interface_method_ref(
        &mut self,
        constant_pool_size: usize,
    ) -> Result<ConstantValue, ClassParserError> {
        let class_index = self.parse_index(constant_pool_size)?;
        let name_and_type_index = self.parse_index(constant_pool_size)?;

        Ok(ConstantValue::InterfaceMethodRef {
            class_index,
            name_and_type_index,
        })
    }

    fn parse_method_ref(
        &mut self,
        constant_pool_size: usize,
    ) -> Result<ConstantValue, ClassParserError> {
        let class_index = self.parse_index(constant_pool_size)?;
        let name_and_type_index = self.parse_index(constant_pool_size)?;

        Ok(ConstantValue::MethodRef {
            class_index,
            name_and_type_index,
        })
    }

    fn parse_field_ref(
        &mut self,
        constant_pool_size: usize,
    ) -> Result<ConstantValue, ClassParserError> {
        let class_index = self.parse_index(constant_pool_size)?;
        let name_and_type_index = self.parse_index(constant_pool_size)?;
        Ok(ConstantValue::FieldRef {
            class_index,
            name_and_type_index,
        })
    }

    fn parse_string(
        &mut self,
        constant_pool_size: usize,
    ) -> Result<ConstantValue, ClassParserError> {
        let index = self.parse_index(constant_pool_size)?;
        Ok(ConstantValue::String { utf8_index: index })
    }

    fn parse_class_const(
        &mut self,
        constant_pool_size: usize,
    ) -> Result<ConstantValue, ClassParserError> {
        let index = self.parse_index(constant_pool_size)?;
        Ok(ConstantValue::Class { name_index: index })
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

    fn parse_index(&mut self, constant_pool_size: usize) -> Result<NonZeroUsize, ClassParserError> {
        let read_index = self.parse_u16()? as usize;
        let index_option = NonZeroUsize::new(read_index);
        if let Some(index) = index_option
            && index.get() < constant_pool_size
        {
            Ok(index)
        } else {
            Err(ClassParserError::InvalidConstantPoolIndex)
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

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CLASS_FILE_PATH: &str = "test_classes/Test.class";

    #[test]
    fn test_parse_ok() {
        let class = parse(TEST_CLASS_FILE_PATH).unwrap();
        assert_eq!("Test", class.get_class_name().unwrap());
        assert_eq!("java/lang/Object", class.get_super_class_name().unwrap());
        assert_eq!(1, class.fields.len());
        assert!(class.interfaces.is_empty());

        let field = &class.fields[0];
        let field_name = class.constant_pool.get_utf8(field.name_index).unwrap();
        assert_eq!("a", field_name);

        let field_type = class
            .constant_pool
            .get_utf8(field.descriptor_index)
            .unwrap();
        assert_eq!("I", field_type);

        assert!(
            field
                .access_flags
                .check_flag(FieldAccessFlags::PRIVATE_FLAG)
        );
        let unexpected_field_flags = [
            FieldAccessFlags::PUBLIC_FLAG,
            FieldAccessFlags::PROTECTED_FLAG,
            FieldAccessFlags::FINAL_FLAG,
            FieldAccessFlags::STATIC_FLAG,
            FieldAccessFlags::SYNTHETIC_FLAG,
            FieldAccessFlags::TRANSIENT_FLAG,
            FieldAccessFlags::VOLATILE_FLAG,
            FieldAccessFlags::ENUM_FLAG,
        ];
        for flag in unexpected_field_flags {
            assert!(!field.access_flags.check_flag(flag))
        }
        assert!(field.attributes.is_empty());

        let method1 = &class.methods[0];
        let method2 = &class.methods[1];
        assert_method_values(method1, &class.constant_pool, "<init>", "()V");
        assert_method_values(method2, &class.constant_pool, "hello", "()V");

        assert!(class.access_flags.check_flag(ClassAccessFlags::PUBLIC_FLAG));
        assert!(class.access_flags.check_flag(ClassAccessFlags::SUPER_FLAG));
        let unexpected_class_flags = [
            ClassAccessFlags::ABSTRACT_FLAG,
            ClassAccessFlags::ANNOTATION_FLAG,
            ClassAccessFlags::SYNTHETIC_FLAG,
            ClassAccessFlags::INTERFACE_FLAG,
            ClassAccessFlags::FINAL_FLAG,
            ClassAccessFlags::ENUM_FLAG,
        ];
        for flag in unexpected_class_flags {
            assert!(!class.access_flags.check_flag(flag));
        }

        assert_eq!(1, class.attributes.len());
        let sourcefile = match &class.attributes[0] {
            Attribute::SourceFile { sourcefile_index } => {
                class.constant_pool.get_utf8(*sourcefile_index).unwrap()
            }
            _ => panic!("expected source file attribute"),
        };
        assert_eq!("Test.java", sourcefile)
    }

    fn assert_method_values(
        method: &Method,
        const_pool: &ConstantPool,
        expected_name: &str,
        expected_descriptor: &str,
    ) {
        let name = const_pool.get_utf8(method.name_index).unwrap();
        assert_eq!(expected_name, name);

        let descriptor = const_pool.get_utf8(method.descriptor_index).unwrap();
        assert_eq!(expected_descriptor, descriptor);

        let code: Vec<_> = method
            .attributes
            .iter()
            .filter_map(|atr| match atr {
                Attribute::Code(c) => Some(c),
                _ => None,
            })
            .collect();

        assert_eq!(1, code.len());
        assert!(!code[0].code.is_empty());
        assert!(code[0].max_stack > 0);
        assert!(code[0].max_locals > 0);

        assert!(
            method
                .access_flags
                .check_flag(MethodAccessFlags::PUBLIC_FLAG)
        );
        let unexpected_field_flags = [
            MethodAccessFlags::PRIVATE_FLAG,
            MethodAccessFlags::PROTECTED_FLAG,
            MethodAccessFlags::FINAL_FLAG,
            MethodAccessFlags::STATIC_FLAG,
            MethodAccessFlags::SYNTHETIC_FLAG,
            MethodAccessFlags::STRICT_FLAG,
            MethodAccessFlags::SYNCHRONIZED_FLAG,
            MethodAccessFlags::NATIVE_FLAG,
            MethodAccessFlags::VARARGS_FLAG,
        ];
        for flag in unexpected_field_flags {
            assert!(!method.access_flags.check_flag(flag));
        }
    }

    #[test]
    fn test_parse_unexpected_end_of_file() {
        let result = parse_from_bytes(vec![0xCA, 0xFE, 0xBA, 0xBE, 0x0]);
        let error = result.unwrap_err();
        assert!(matches!(error, ClassParserError::UnexpectedEndOfFile));
    }

    #[test]
    fn test_parse_invalid_magic_number() {
        let result = parse_from_bytes(vec![0xCA, 0xFE, 0x01, 0xBE, 0x0]);
        let error = result.unwrap_err();
        assert!(matches!(error, ClassParserError::InvalidMagicNumber));
    }

    #[test]
    fn test_parse_empty_file() {
        let result = parse_from_bytes(vec![]);
        let error = result.unwrap_err();
        assert!(matches!(error, ClassParserError::EmptyFile));
    }

    #[test]
    fn test_parse_trailing_bytes() {
        let mut bytes = std::fs::read(TEST_CLASS_FILE_PATH).unwrap();
        bytes.push(0);
        let result = parse_from_bytes(bytes);
        let error = result.unwrap_err();
        assert!(matches!(error, ClassParserError::ExpectedEndOfFile));
    }
}
