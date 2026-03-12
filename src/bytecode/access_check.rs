use std::rc::Rc;

use crate::{
    class_file::{FieldAccessFlags, MethodAccessFlags},
    jvm_model::JvmClass,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessLevel {
    Private,
    Public,
    Protected,
    Package,
}
impl From<MethodAccessFlags> for AccessLevel {
    fn from(value: MethodAccessFlags) -> Self {
        if value.check_flag(MethodAccessFlags::PUBLIC_FLAG) {
            Self::Public
        } else if value.check_flag(MethodAccessFlags::PRIVATE_FLAG) {
            Self::Private
        } else if value.check_flag(MethodAccessFlags::PROTECTED_FLAG) {
            Self::Protected
        } else {
            Self::Package
        }
    }
}
impl From<FieldAccessFlags> for AccessLevel {
    fn from(value: FieldAccessFlags) -> Self {
        if value.check_flag(FieldAccessFlags::PUBLIC_FLAG) {
            Self::Public
        } else if value.check_flag(FieldAccessFlags::PRIVATE_FLAG) {
            Self::Private
        } else if value.check_flag(FieldAccessFlags::PROTECTED_FLAG) {
            Self::Protected
        } else {
            Self::Package
        }
    }
}

/// if current class doesn't have access to the target resource
/// throws an IllegalAccessError ($size is the size of the instruction)
/// # Parameters
/// - `$current_class`: The JvmClass that is attempting to access the resource (caller class).
/// - `$target_class`: The JvmClass that owns the resource being accessed (target class).
/// - `$access_flags`: Flags indicating the access level required.
/// - `$frame`: The current JVM frame where the access attempt occurs, used for throwing exceptions.
/// - `$context`: Jvm runtime context needed for exception handling.
/// - `$size`: The size of the instruction needed for the exception throwing.
#[macro_export]
macro_rules! validate_access {
    ($current_class:expr, $target_class:expr, $access_flags:expr, $frame:expr, $context:expr, $size:expr) => {{
        if !$crate::bytecode::access_check::check_has_access(
            &$current_class,
            &$target_class,
            $access_flags,
        ) {
            $crate::throw_illegal_access_error!($frame, $context, $size);
        }
    }};
}

pub fn check_has_access(
    current_class: &Rc<JvmClass>,
    target_class: &Rc<JvmClass>,
    access: impl Into<AccessLevel>,
) -> bool {
    match access.into() {
        AccessLevel::Public => true,
        AccessLevel::Private => Rc::ptr_eq(current_class, target_class),
        AccessLevel::Protected => {
            JvmClass::is_sublcass_of(target_class, current_class)
                || check_same_package(current_class, target_class)
        }
        AccessLevel::Package => check_same_package(current_class, target_class),
    }
}

fn check_same_package(current_class: &JvmClass, target_class: &JvmClass) -> bool {
    let package1 = current_class.class_file.get_package_name();
    let package2 = target_class.class_file.get_package_name();

    package1 == package2
}
