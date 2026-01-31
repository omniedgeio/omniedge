//! WASM Sandbox for plugin isolation
//!
//! Uses wasmtime with the Component Model to provide memory-safe
//! sandboxed execution of plugins with resource limits.

use crate::error::{PluginError, PluginResult};
use crate::host::HostState;
use std::path::Path;
use wasmtime::component::Component;
use wasmtime::*;

/// Resource limits for plugin execution
#[derive(Debug, Clone)]
pub struct SandboxLimits {
    /// Maximum memory in bytes (default: 64MB)
    pub max_memory_bytes: u64,
    /// Maximum table elements (default: 10,000)
    pub max_table_elements: u32,
    /// Maximum instances (default: 10)
    pub max_instances: u32,
    /// Maximum execution time in milliseconds (default: 100ms)
    pub max_execution_time_ms: u64,
    /// Maximum fuel (for deterministic limiting)
    pub max_fuel: u64,
}

impl Default for SandboxLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 64 * 1024 * 1024, // 64MB
            max_table_elements: 10_000,
            max_instances: 10,
            max_execution_time_ms: 100,
            max_fuel: 1_000_000,
        }
    }
}

/// State held by the WASM store
pub struct PluginStoreState {
    /// Host state for plugin callbacks
    pub host: HostState,
    /// Resource limiter
    limits: StoreLimits,
}

impl PluginStoreState {
    /// Create new store state
    pub fn new(host: HostState, limits: SandboxLimits) -> Self {
        Self {
            host,
            limits: StoreLimitsBuilder::new()
                .memory_size(limits.max_memory_bytes as usize)
                .table_elements(limits.max_table_elements)
                .instances(limits.max_instances as usize)
                .build(),
        }
    }
}

/// WASM sandbox for plugin execution
pub struct PluginSandbox {
    /// Wasmtime engine (shared across all plugins)
    engine: Engine,
    /// Linker with host functions
    linker: Linker<PluginStoreState>,
    /// Resource limits
    limits: SandboxLimits,
}

impl PluginSandbox {
    /// Create a new plugin sandbox
    pub fn new(limits: SandboxLimits) -> PluginResult<Self> {
        let mut config = Config::new();

        // Enable component model for WIT-based interfaces
        config.wasm_component_model(true);

        // Enable fuel for deterministic execution limits
        config.consume_fuel(true);

        // Async support for non-blocking plugin calls
        config.async_support(true);

        // Cranelift optimizations
        config.cranelift_opt_level(OptLevel::Speed);

        let engine = Engine::new(&config)?;
        let linker = Linker::new(&engine);

        Ok(Self {
            engine,
            linker,
            limits,
        })
    }

    /// Create a sandbox with default limits
    pub fn with_defaults() -> PluginResult<Self> {
        Self::new(SandboxLimits::default())
    }

    /// Get a reference to the engine
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Get a mutable reference to the linker
    pub fn linker_mut(&mut self) -> &mut Linker<PluginStoreState> {
        &mut self.linker
    }

    /// Link host functions to the sandbox
    pub fn link_host_functions(&mut self) -> PluginResult<()> {
        // Link logging functions
        self.linker.func_wrap(
            "omniedge:host/logging",
            "log-trace",
            |mut caller: Caller<'_, PluginStoreState>, message_ptr: i32, message_len: i32| {
                let memory = caller.get_export("memory").and_then(|e| e.into_memory());
                if let Some(memory) = memory {
                    let data = memory.data(&caller);
                    if let Some(slice) =
                        data.get(message_ptr as usize..(message_ptr + message_len) as usize)
                    {
                        if let Ok(message) = std::str::from_utf8(slice) {
                            crate::host::host_log_trace(&caller.data().host, message);
                        }
                    }
                }
            },
        )?;

        self.linker.func_wrap(
            "omniedge:host/logging",
            "log-debug",
            |mut caller: Caller<'_, PluginStoreState>, message_ptr: i32, message_len: i32| {
                let memory = caller.get_export("memory").and_then(|e| e.into_memory());
                if let Some(memory) = memory {
                    let data = memory.data(&caller);
                    if let Some(slice) =
                        data.get(message_ptr as usize..(message_ptr + message_len) as usize)
                    {
                        if let Ok(message) = std::str::from_utf8(slice) {
                            crate::host::host_log_debug(&caller.data().host, message);
                        }
                    }
                }
            },
        )?;

        self.linker.func_wrap(
            "omniedge:host/logging",
            "log-info",
            |mut caller: Caller<'_, PluginStoreState>, message_ptr: i32, message_len: i32| {
                let memory = caller.get_export("memory").and_then(|e| e.into_memory());
                if let Some(memory) = memory {
                    let data = memory.data(&caller);
                    if let Some(slice) =
                        data.get(message_ptr as usize..(message_ptr + message_len) as usize)
                    {
                        if let Ok(message) = std::str::from_utf8(slice) {
                            crate::host::host_log_info(&caller.data().host, message);
                        }
                    }
                }
            },
        )?;

        self.linker.func_wrap(
            "omniedge:host/logging",
            "log-warn",
            |mut caller: Caller<'_, PluginStoreState>, message_ptr: i32, message_len: i32| {
                let memory = caller.get_export("memory").and_then(|e| e.into_memory());
                if let Some(memory) = memory {
                    let data = memory.data(&caller);
                    if let Some(slice) =
                        data.get(message_ptr as usize..(message_ptr + message_len) as usize)
                    {
                        if let Ok(message) = std::str::from_utf8(slice) {
                            crate::host::host_log_warn(&caller.data().host, message);
                        }
                    }
                }
            },
        )?;

        self.linker.func_wrap(
            "omniedge:host/logging",
            "log-error",
            |mut caller: Caller<'_, PluginStoreState>, message_ptr: i32, message_len: i32| {
                let memory = caller.get_export("memory").and_then(|e| e.into_memory());
                if let Some(memory) = memory {
                    let data = memory.data(&caller);
                    if let Some(slice) =
                        data.get(message_ptr as usize..(message_ptr + message_len) as usize)
                    {
                        if let Ok(message) = std::str::from_utf8(slice) {
                            crate::host::host_log_error(&caller.data().host, message);
                        }
                    }
                }
            },
        )?;

        // Link time functions
        self.linker
            .func_wrap("omniedge:host/time", "now-ms", || -> u64 {
                crate::host::host_time_now_ms()
            })?;

        self.linker
            .func_wrap("omniedge:host/time", "now-ns", || -> u64 {
                crate::host::host_time_now_ns()
            })?;

        Ok(())
    }

    /// Create a new store with the given host state
    pub fn create_store(&self, host_state: HostState) -> Store<PluginStoreState> {
        let state = PluginStoreState::new(host_state, self.limits.clone());
        let mut store = Store::new(&self.engine, state);

        // Set resource limiter
        store.limiter(|s| &mut s.limits);

        // Set initial fuel
        let _ = store.set_fuel(self.limits.max_fuel);

        store
    }

    /// Compile a WASM module from bytes
    pub fn compile_module(&self, wasm_bytes: &[u8]) -> PluginResult<Module> {
        Module::new(&self.engine, wasm_bytes)
            .map_err(|e| PluginError::CompilationError(e.to_string()))
    }

    /// Compile a WASM module from a file
    pub fn compile_module_from_file(&self, path: &Path) -> PluginResult<Module> {
        Module::from_file(&self.engine, path)
            .map_err(|e| PluginError::CompilationError(e.to_string()))
    }

    /// Compile a WASM component from bytes
    pub fn compile_component(&self, wasm_bytes: &[u8]) -> PluginResult<Component> {
        Component::new(&self.engine, wasm_bytes)
            .map_err(|e| PluginError::CompilationError(e.to_string()))
    }

    /// Compile a WASM component from a file
    pub fn compile_component_from_file(&self, path: &Path) -> PluginResult<Component> {
        Component::from_file(&self.engine, path)
            .map_err(|e| PluginError::CompilationError(e.to_string()))
    }

    /// Instantiate a module in a store
    pub fn instantiate_module(
        &self,
        store: &mut Store<PluginStoreState>,
        module: &Module,
    ) -> PluginResult<Instance> {
        self.linker
            .instantiate(store, module)
            .map_err(|e| PluginError::InstantiationError(e.to_string()))
    }

    /// Get remaining fuel in a store
    pub fn remaining_fuel(store: &Store<PluginStoreState>) -> Option<u64> {
        store.get_fuel().ok()
    }

    /// Check if store has run out of fuel
    pub fn is_out_of_fuel(store: &Store<PluginStoreState>) -> bool {
        store.get_fuel().map(|f| f == 0).unwrap_or(false)
    }
}

/// A loaded plugin instance
pub struct PluginInstance {
    /// The compiled module
    module: Module,
    /// Plugin ID
    plugin_id: String,
}

impl PluginInstance {
    /// Create a new plugin instance
    pub fn new(module: Module, plugin_id: impl Into<String>) -> Self {
        Self {
            module,
            plugin_id: plugin_id.into(),
        }
    }

    /// Get the plugin ID
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    /// Get the module
    pub fn module(&self) -> &Module {
        &self.module
    }

    /// Create an instance in a store
    pub fn instantiate(
        &self,
        sandbox: &PluginSandbox,
        store: &mut Store<PluginStoreState>,
    ) -> PluginResult<Instance> {
        sandbox.instantiate_module(store, &self.module)
    }
}

/// Builder for configuring sandbox limits
pub struct SandboxBuilder {
    limits: SandboxLimits,
}

impl SandboxBuilder {
    /// Create a new builder with default limits
    pub fn new() -> Self {
        Self {
            limits: SandboxLimits::default(),
        }
    }

    /// Set maximum memory
    pub fn max_memory(mut self, bytes: u64) -> Self {
        self.limits.max_memory_bytes = bytes;
        self
    }

    /// Set maximum table elements
    pub fn max_table_elements(mut self, count: u32) -> Self {
        self.limits.max_table_elements = count;
        self
    }

    /// Set maximum instances
    pub fn max_instances(mut self, count: u32) -> Self {
        self.limits.max_instances = count;
        self
    }

    /// Set maximum execution time
    pub fn max_execution_time_ms(mut self, ms: u64) -> Self {
        self.limits.max_execution_time_ms = ms;
        self
    }

    /// Set maximum fuel
    pub fn max_fuel(mut self, fuel: u64) -> Self {
        self.limits.max_fuel = fuel;
        self
    }

    /// Build the sandbox
    pub fn build(self) -> PluginResult<PluginSandbox> {
        PluginSandbox::new(self.limits)
    }
}

impl Default for SandboxBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_creation() {
        let sandbox = PluginSandbox::with_defaults();
        assert!(sandbox.is_ok());
    }

    #[test]
    fn test_sandbox_builder() {
        let sandbox = SandboxBuilder::new()
            .max_memory(32 * 1024 * 1024) // 32MB
            .max_fuel(500_000)
            .build();

        assert!(sandbox.is_ok());
    }

    #[test]
    fn test_store_creation() {
        let sandbox = PluginSandbox::with_defaults().unwrap();
        let host_state = HostState::new("test-plugin");
        let store = sandbox.create_store(host_state);

        // Check fuel was set
        let fuel = store.get_fuel();
        assert!(fuel.is_ok());
    }
}
