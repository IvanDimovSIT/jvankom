/// initialises the class and rewinds the instruction, where $size is the size of the instruction
#[macro_export]
macro_rules! initialise_class_and_rewind {
    ($frame:expr, $context:expr, $jvm_class:expr, $size:expr) => {{
        const _CHECK_SIZE: () = assert!($size > 0);

        $frame.program_counter -= $size; // rewind
        return JVM::initialise_class(
            $context.current_thread,
            $jvm_class,
            $context.class_loader,
            $jvm_class.class_file.get_class_name().unwrap(),
        );
    }};
}

/// throws a NullPointerException, $size is the size of the instruction
#[macro_export]
macro_rules! throw_null_pointer_exception {
    ($frame:expr, $context:expr, $size:expr) => {{
        const _CHECK_SIZE: () = assert!($size > 0);

        $frame.program_counter -= $size - 1; // rewind
        return $crate::exceptions::throw_jvm_exception(
            $context.current_thread,
            $context.heap,
            $context.class_loader,
            $crate::jvm_model::NULL_POINTER_EXCEPTION_NAME,
        );
    }};
}

/// throws a ArrayIndexOutOfBoundsException, $size is the size of the instruction
#[macro_export]
macro_rules! throw_array_index_out_of_bounds_exception {
    ($frame:expr, $context:expr, $size:expr) => {{
        const _CHECK_SIZE: () = assert!($size > 0);

        $frame.program_counter -= $size - 1; // rewind
        return $crate::exceptions::throw_jvm_exception(
            $context.current_thread,
            $context.heap,
            $context.class_loader,
            $crate::jvm_model::ARRAY_INDEX_OUT_OF_BOUNDS_EXCEPTION_NAME,
        );
    }};
}

///  throws a NegativeArraySizeException, $size is the size of the instruction
#[macro_export]
macro_rules! throw_negative_array_size_exception {
    ($frame:expr, $context:expr, $size:expr) => {{
        const _CHECK_SIZE: () = assert!($size > 0);

        $frame.program_counter -= $size - 1; // rewind
        return $crate::exceptions::throw_jvm_exception(
            $context.current_thread,
            $context.heap,
            $context.class_loader,
            $crate::jvm_model::NEGATIVE_ARRAY_SIZE_EXCEPTION_NAME,
        );
    }};
}

///  throws a ArrayStoreException, $size is the size of the instruction
#[macro_export]
macro_rules! throw_array_store_exception {
    ($frame:expr, $context:expr, $size:expr) => {{
        const _CHECK_SIZE: () = assert!($size > 0);

        $frame.program_counter -= $size - 1; // rewind
        return $crate::exceptions::throw_jvm_exception(
            $context.current_thread,
            $context.heap,
            $context.class_loader,
            $crate::jvm_model::ARRAY_STORE_EXCEPTION_NAME,
        );
    }};
}
