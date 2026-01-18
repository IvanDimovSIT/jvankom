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
    /// placeholder after long and double
    Unusable,
}

pub struct Bytecode {
    pub code: Vec<u8>,
    pub max_stack: u32,
    pub max_locals: u32,
}

pub struct Field {
    pub name: String,
    pub descriptor: String,
    pub access_flags: u32,
    pub offset: u32,
}

pub struct Method {
    pub name: String,
    pub descriptor: String,
    pub access_flags: u32,
    pub bytecode: Option<Bytecode>,
}

pub struct ClassFile {
    pub class_index: usize,
    pub super_class_index: Option<usize>,
    pub interfaces: Vec<usize>,
    pub constant_pool: Vec<ConstantValue>,
    pub methods: Vec<Method>,
    pub fields: Vec<Field>,
    pub access_flags: u32,
}
