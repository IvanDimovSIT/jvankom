use std::num::NonZeroUsize;

#[derive(Debug, Clone)]
pub enum ConstantValue {
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Utf8(String),
    Class {
        name_index: NonZeroUsize,
    },
    String {
        utf8_index: NonZeroUsize,
    },
    NameAndType {
        name_index: NonZeroUsize,
        descriptor_index: NonZeroUsize,
    },
    FieldRef {
        class_index: NonZeroUsize,
        name_and_type_index: NonZeroUsize,
    },
    MethodRef {
        class_index: NonZeroUsize,
        name_and_type_index: NonZeroUsize,
    },
    InterfaceMethodRef {
        class_index: NonZeroUsize,
        name_and_type_index: NonZeroUsize,
    },
    MethodHandle {
        reference_kind: u8,
        reference_index: NonZeroUsize,
    },
    MethodType {
        descriptor_index: NonZeroUsize,
    },
    InvokeDynamic {
        bootstrap_method_attr_index: NonZeroUsize,
        name_and_type_index: NonZeroUsize,
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
    ConstantValue {
        value_index: NonZeroUsize,
    },
    SourceFile {
        sourcefile_index: NonZeroUsize,
    },
    Unknown {
        name_index: NonZeroUsize,
        info: Vec<u8>,
    },
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name_index: NonZeroUsize,
    pub descriptor_index: NonZeroUsize,
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
pub struct ConstantPool {
    constant_pool_table: Vec<ConstantValue>,
}
impl ConstantPool {
    pub fn new(constant_pool_table: Vec<ConstantValue>) -> Self {
        Self {
            constant_pool_table,
        }
    }

    pub fn len(&self) -> usize {
        self.constant_pool_table.len()
    }

    pub fn get(&self, index: NonZeroUsize) -> &ConstantValue {
        &self.constant_pool_table[index.get()]
    }

    pub fn get_utf8(&self, index: NonZeroUsize) -> Option<&str> {
        match self.get(index) {
            ConstantValue::Utf8(s) => Some(s),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClassFile {
    pub class_index: NonZeroUsize,
    pub super_class_index: Option<NonZeroUsize>,
    pub interfaces: Vec<NonZeroUsize>,
    pub constant_pool: ConstantPool,
    pub methods: Vec<Method>,
    pub fields: Vec<Field>,
    pub access_flags: u16,
    pub attributes: Vec<Attribute>,
}
