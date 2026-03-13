use crate::jvm_cache::{
    method_call_cache::MethodCallCache, method_signature_cache::MethodSignatureCache,
    string_pool::StringPool,
};

pub mod method_call_cache;
pub mod method_signature_cache;
pub mod string_pool;

/// JVM-wide runtime cached information
#[derive(Debug)]
pub struct JvmCache {
    pub method_call_cache: MethodCallCache,
    pub string_pool: StringPool,
    pub method_signature_cache: MethodSignatureCache,
}
impl JvmCache {
    pub fn new() -> Self {
        Self {
            method_call_cache: MethodCallCache::new(),
            string_pool: StringPool::new(),
            method_signature_cache: MethodSignatureCache::new(),
        }
    }
}
