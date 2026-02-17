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
pub struct InnerClass {
    pub inner_class_info_index: NonZeroUsize,
    pub outer_class_info_index: NonZeroUsize,
    pub inner_name_index: Option<NonZeroUsize>,
    pub inner_class_access_flags: ClassAccessFlags,
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
    InnerClasses(Vec<InnerClass>),
    Unknown {
        name_index: NonZeroUsize,
        info: Vec<u8>,
    },
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name_index: NonZeroUsize,
    pub descriptor_index: NonZeroUsize,
    pub access_flags: FieldAccessFlags,
    pub attributes: Vec<Attribute>,
}

#[derive(Debug, Clone, Copy)]
pub struct FieldAccessFlags {
    access_flags: u16,
}
impl FieldAccessFlags {
    pub const PUBLIC_FLAG: u16 = 0x0001;
    pub const PRIVATE_FLAG: u16 = 0x0002;
    pub const PROTECTED_FLAG: u16 = 0x0004;
    pub const STATIC_FLAG: u16 = 0x0008;
    pub const FINAL_FLAG: u16 = 0x0010;
    pub const VOLATILE_FLAG: u16 = 0x0040;
    pub const TRANSIENT_FLAG: u16 = 0x0080;
    pub const SYNTHETIC_FLAG: u16 = 0x1000;
    pub const ENUM_FLAG: u16 = 0x4000;

    pub fn new(access_flags: u16) -> Option<Self> {
        let flags = Self { access_flags };
        let mut access_count = 0;
        if flags.check_flag(Self::PUBLIC_FLAG) {
            access_count += 1;
        }
        if flags.check_flag(Self::PROTECTED_FLAG) {
            access_count += 1;
        }
        if flags.check_flag(Self::PRIVATE_FLAG) {
            access_count += 1;
        }
        if access_count > 1 {
            return None;
        }

        let is_invalid_combination =
            flags.check_flag(Self::VOLATILE_FLAG) && flags.check_flag(Self::FINAL_FLAG);
        if is_invalid_combination {
            return None;
        }

        Some(flags)
    }

    pub fn check_flag(self, flag: u16) -> bool {
        self.access_flags & flag != 0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MethodAccessFlags {
    access_flags: u16,
}
impl MethodAccessFlags {
    pub const PUBLIC_FLAG: u16 = 0x0001;
    pub const PRIVATE_FLAG: u16 = 0x0002;
    pub const PROTECTED_FLAG: u16 = 0x0004;
    pub const STATIC_FLAG: u16 = 0x0008;
    pub const FINAL_FLAG: u16 = 0x0010;
    pub const SYNCHRONIZED_FLAG: u16 = 0x0020;
    pub const BRIDGE_FLAG: u16 = 0x0040;
    pub const VARARGS_FLAG: u16 = 0x0080;
    pub const NATIVE_FLAG: u16 = 0x0100;
    pub const ABSTRACT_FLAG: u16 = 0x0400;
    pub const STRICT_FLAG: u16 = 0x0800;
    pub const SYNTHETIC_FLAG: u16 = 0x1000;

    pub fn new(access_flags: u16) -> Option<Self> {
        let flags = Self { access_flags };
        let mut access_count = 0;
        if flags.check_flag(Self::PUBLIC_FLAG) {
            access_count += 1;
        }
        if flags.check_flag(Self::PROTECTED_FLAG) {
            access_count += 1;
        }
        if flags.check_flag(Self::PRIVATE_FLAG) {
            access_count += 1;
        }
        if access_count > 1 {
            return None;
        }

        let is_invalid_combination = flags.check_flag(Self::ABSTRACT_FLAG)
            && (flags.check_flag(Self::FINAL_FLAG)
                || flags.check_flag(Self::NATIVE_FLAG)
                || flags.check_flag(Self::STATIC_FLAG)
                || flags.check_flag(Self::SYNCHRONIZED_FLAG)
                || flags.check_flag(Self::STRICT_FLAG)
                || flags.check_flag(Self::PRIVATE_FLAG));
        if is_invalid_combination {
            return None;
        }

        Some(flags)
    }

    pub fn check_flag(self, flag: u16) -> bool {
        self.access_flags & flag != 0
    }
}

#[derive(Debug, Clone)]
pub struct Method {
    pub name_index: NonZeroUsize,
    pub descriptor_index: NonZeroUsize,
    pub access_flags: MethodAccessFlags,
    pub attributes: Vec<Attribute>,
}
impl Method {
    pub fn get_bytecode(&self, index: usize) -> &Bytecode {
        match &self.attributes[index] {
            Attribute::Code(bytecode) => bytecode,
            _ => panic!("Expected code attribute"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConstantPool {
    constant_pool_table: Vec<ConstantValue>,
}
impl ConstantPool {
    pub fn new(constant_pool_table: Vec<ConstantValue>) -> Self {
        assert!(!constant_pool_table.is_empty());
        Self {
            constant_pool_table,
        }
    }

    pub fn get_all_constants(&self) -> &[ConstantValue] {
        &self.constant_pool_table
    }

    pub fn len(&self) -> usize {
        self.constant_pool_table.len()
    }

    pub fn get(&self, index: NonZeroUsize) -> &ConstantValue {
        debug_assert!(index.get() < self.constant_pool_table.len());
        &self.constant_pool_table[index.get()]
    }

    pub fn get_utf8(&self, index: NonZeroUsize) -> Option<&str> {
        match self.get(index) {
            ConstantValue::Utf8(s) => Some(s),
            _ => None,
        }
    }

    /// returns the class, method name and method descriptor based on method ref index
    pub fn get_class_methodname_descriptor(
        &self,
        method_ref_index: NonZeroUsize,
    ) -> Option<(&str, &str, &str)> {
        let (class_i, name_and_type_i) = self.get_method_ref(method_ref_index)?;
        let class = self.get_class_name(class_i)?;
        let (method_name, method_descriptor) = self.get_name_and_type(name_and_type_i)?;
        Some((class, method_name, method_descriptor))
    }

    pub fn get_method_ref(&self, index: NonZeroUsize) -> Option<(NonZeroUsize, NonZeroUsize)> {
        match self.get(index) {
            ConstantValue::MethodRef {
                class_index,
                name_and_type_index,
            } => Some((*class_index, *name_and_type_index)),
            _ => None,
        }
    }

    /// returns (name, descriptor)
    pub fn get_name_and_type(&self, name_and_type_index: NonZeroUsize) -> Option<(&str, &str)> {
        match self.get(name_and_type_index) {
            ConstantValue::NameAndType {
                name_index,
                descriptor_index,
            } => {
                let name = self.get_utf8(*name_index)?;
                let descriptor = self.get_utf8(*descriptor_index)?;
                Some((name, descriptor))
            }
            _ => None,
        }
    }

    pub fn get_class_name(&self, class_index: NonZeroUsize) -> Option<&str> {
        let class = self.get(class_index);
        let name_index = match class {
            ConstantValue::Class { name_index } => name_index,
            _ => return None,
        };
        let class_name = self.get_utf8(*name_index)?;

        Some(class_name)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ClassAccessFlags {
    access_flags: u16,
}
impl ClassAccessFlags {
    pub const PUBLIC_FLAG: u16 = 0x0001;
    pub const FINAL_FLAG: u16 = 0x0010;
    pub const SUPER_FLAG: u16 = 0x0020;
    pub const INTERFACE_FLAG: u16 = 0x0200;
    pub const ABSTRACT_FLAG: u16 = 0x0400;
    pub const SYNTHETIC_FLAG: u16 = 0x1000;
    pub const ANNOTATION_FLAG: u16 = 0x2000;
    pub const ENUM_FLAG: u16 = 0x4000;

    pub fn new(access_flags: u16) -> Option<Self> {
        let flags = Self { access_flags };
        if flags.check_flag(Self::INTERFACE_FLAG) && !flags.check_flag(Self::ABSTRACT_FLAG) {
            return None;
        }
        if flags.check_flag(Self::ANNOTATION_FLAG) && !flags.check_flag(Self::INTERFACE_FLAG) {
            return None;
        }
        if flags.check_flag(Self::ENUM_FLAG) && flags.check_flag(Self::FINAL_FLAG) {
            return None;
        }

        Some(flags)
    }

    pub fn check_flag(self, flag: u16) -> bool {
        self.access_flags & flag != 0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ClassFileVersion {
    pub major: u16,
    pub minor: u16,
}

#[derive(Debug, Clone)]
pub struct ClassFile {
    pub version: ClassFileVersion,
    pub class_index: NonZeroUsize,
    pub super_class_index: Option<NonZeroUsize>,
    pub interfaces: Vec<NonZeroUsize>,
    pub constant_pool: ConstantPool,
    pub methods: Vec<Method>,
    pub fields: Vec<Field>,
    pub access_flags: ClassAccessFlags,
    pub attributes: Vec<Attribute>,
}
impl ClassFile {
    pub fn get_class_name(&self) -> Option<&str> {
        self.constant_pool.get_class_name(self.class_index)
    }

    pub fn get_super_class_name(&self) -> Option<&str> {
        self.constant_pool.get_class_name(self.super_class_index?)
    }

    pub fn get_method_and_bytecode_index(
        &self,
        method_name: &str,
        descriptor: &str,
    ) -> Option<(usize, Option<usize>)> {
        for (index, method) in self.methods.iter().enumerate() {
            let methods_match = self.constant_pool.get_utf8(method.name_index)? == method_name
                && self.constant_pool.get_utf8(method.descriptor_index)? == descriptor;
            if methods_match {
                let bytecode_index = method
                    .attributes
                    .iter()
                    .enumerate()
                    .find(|(_, atr)| matches!(atr, Attribute::Code(_)))
                    .map(|x| x.0);

                return Some((index, bytecode_index));
            }
        }

        None
    }
}
