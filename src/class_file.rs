use std::num::NonZeroUsize;

#[derive(Debug, Clone)]
pub enum ConstantValue {
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Utf8(String),
    Class {
        name_index: usize,
    },
    String {
        utf8_index: usize,
    },
    NameAndType {
        name_index: usize,
        descriptor_index: usize,
    },
    FieldRef {
        class_index: usize,
        name_and_type_index: usize,
    },
    MethodRef {
        class_index: usize,
        name_and_type_index: usize,
    },
    InterfaceMethodRef {
        class_index: usize,
        name_and_type_index: usize,
    },
    MethodHandle {
        reference_kind: u8,
        reference_index: usize,
    },
    MethodType {
        descriptor_index: usize,
    },
    InvokeDynamic {
        bootstrap_method_attr_index: usize,
        name_and_type_index: usize,
    },
    /// placeholder after long and double
    Unusable,
}

#[derive(Debug, Clone)]
pub struct ExceptionTableEntry {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    pub catch_type: u16,
}

#[derive(Debug, Clone)]
pub struct Bytecode {
    pub code: Vec<u8>,
    pub max_stack: u16,
    pub max_locals: u16,
    pub exception_table: Vec<ExceptionTableEntry>,
    pub attributes: Vec<Attribute>,
}

#[derive(Debug, Clone)]
pub enum Attribute {
    Code(Bytecode),
    ConstantValue { value_index: usize },
    SourceFile { sourcefile_index: usize },
    Unknown { name_index: usize, info: Vec<u8> },
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name_index: usize,
    pub descriptor_index: usize,
    pub access_flags: u16,
    pub attributes: Vec<Attribute>,
}

#[derive(Debug, Clone)]
pub struct Method {
    pub name: String,
    pub descriptor: String,
    pub access_flags: u32,
    pub bytecode: Option<Bytecode>,
}

#[derive(Debug, Clone)]
pub struct ClassFile {
    pub class_index: usize,
    pub super_class_index: Option<NonZeroUsize>,
    pub interfaces: Vec<usize>,
    pub constant_pool: Vec<ConstantValue>,
    pub methods: Vec<Method>,
    pub fields: Vec<Field>,
    pub access_flags: u16,
    pub attributes: Vec<Attribute>,
}
