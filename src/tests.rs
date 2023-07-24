use crate::{
    env::{envs, Environment},
    env_runtime,
    jvm::Jvm,
    runtime::Runtime,
    stack::Stack,
    Result,
};
use convert_case::{Case, Casing};
use jni::{InitArgsBuilder, JNIVersion, JavaVM};
use std::str;
use wasmi::{core::Value, Caller, Func, Store};

/// Converts the given `.wat` into `.wasm`.
pub fn wat2wasm(wat: &str) -> Result<Vec<u8>, wat::Error> {
    wat::parse_str(wat)
}

// Test implementation of JVM calls from the stack
impl Jvm for Stack {
    fn jvm_get_bytecode(&self, _name: &str) -> Result<Vec<u8>> {
        let wat = r#"
        (module
            (func (export "_constructor"))
            (func (export "sum") (param $p0 i64) (result i32)
                (i32.ne
                    (i32.add
                        (i32.const 2)
                        (i32.wrap_i64
                            (local.get $p0)))
                    (i32.const 4))
            )
        )
        "#;

        Ok(wat2wasm(wat).expect("WAT code parsing failed"))
    }
}

env_runtime! {
    #[version = 0]
    pub fn TestSetValue(value: u32) {
        |mut _caller: Caller<Runtime>| {
            assert_eq!(42, value);
        }
    }
}

env_runtime! {
    #[version = 0]
    pub fn TestGetValue() -> u32 {
        |mut _caller: Caller<Runtime>| {
            42
        }
    }
}

env_runtime! {
    #[version = 0]
    pub fn TestMemory(offset: u32, length: u32) {
        |mut caller: Caller<Runtime>| {
            let (memory, _ctx) = caller
                .data()
                .memory()
                .expect("Error get memory")
                .data_and_store_mut(&mut caller);

            let result = str::from_utf8(&memory[offset as usize..offset as usize + length as usize])
                .expect("Error converts a slice of bytes to a string slice");

            assert_eq!("Hi", result);
        }
    }
}

struct TestRunner {
    java_vm: JavaVM,
}

impl TestRunner {
    pub fn new() -> Self {
        // Preparing a fake jvm to initialize the call stack
        let jvm_args = InitArgsBuilder::new()
            .version(JNIVersion::V8)
            .option("-Xcheck:jni")
            .build()
            .expect("Failed to initialize JVM args");
        let java_vm = JavaVM::new(jvm_args).expect("JavaVM initialization failed");

        Self { java_vm }
    }

    pub fn run(&self, wat: &str, memory: Option<(u32, u32)>, input_data: Vec<u8>) -> Vec<Value> {
        // Preparing a fake jvm to initialize the call stack
        let env = self
            .java_vm
            .attach_current_thread()
            .expect("Failed attaches the current thread to the Java VM");

        let jvm = env
            .get_java_vm()
            .expect("Failed receiving JavaVM interface");
        let array = env.new_byte_array(1).expect("Array creation failed");
        let global_ref = env
            .new_global_ref(array)
            .expect("Error callback new_global_ref");

        let bytecode = wat2wasm(wat).expect("WAT code parsing failed");
        // If the size of the allocated memory is not specified, we output the minimum value
        let memory = match memory {
            Some(mem) => mem,
            None => (1, 1),
        };

        // TODO: Maybe not all the environments are needed
        let envs = [envs(), Self::test_envs()].concat();

        let mut stack = Stack::new(bytecode, memory, envs, jvm, global_ref)
            .expect("Call stack creation failed");

        stack
            .run("_constructor", input_data)
            .expect("Bytecode execution failed")
    }

    /// Test imports
    fn test_envs() -> Vec<Box<dyn Environment>> {
        let test_set_value = TestSetValue;
        let test_get_value = TestGetValue;
        let test_memory = TestMemory;

        vec![
            Box::new(test_set_value),
            Box::new(test_get_value),
            Box::new(test_memory),
        ]
    }
}

#[test]
fn test_vm() {
    let runner = TestRunner::new();

    {
        let wat = r#"
        (module
            (func (export "_constructor") (result i32)
                (i32.add
                    (i32.const 2)
                    (i32.const 2))
            )
        )
        "#;

        let result = runner.run(wat, None, vec![]);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Value::I32(4));
    }

    {
        let wat = r#"
        (module
            (import "env0" "test_get_value" (func $test_get_value (result i32)))

            (func (export "_constructor") (result i32)
                (call $test_get_value)
            )
        )
        "#;

        let result = runner.run(wat, None, vec![]);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Value::I32(42));
    }

    {
        let wat = r#"
        (module
            (import "env0" "test_set_value" (func $test_set_value (param i32)))

            (func (export "_constructor") (result i32)
                (call $test_set_value
                    (i32.const 42))

                (i32.const 0)
            )
        )
        "#;

        let result = runner.run(wat, None, vec![]);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Value::I32(0));
    }

    {
        let wat = r#"
        (module
            (import "env0" "test_memory" (func $test_memory (param i32 i32)))

            (import "env" "memory" (memory 1 1))

            (func (export "_constructor") (result i32)
                (call $test_memory
                    (i32.const 0)  ;; Pass offset 0 to test
                    (i32.const 2)) ;; Pass length 2 to test

                (i32.const 0)
            )

            ;; For test memory
            (data (i32.const 0) "Hi")
        )
        "#;

        let result = runner.run(wat, None, vec![]);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Value::I32(0));
    }

    {
        let wat = r#"
        (module
            (import "env0" "call_contract" (func $call (param i32 i32 i32 i32 i32 i32) (result i32)))

            (import "env" "memory" (memory 1 1))

            (func (export "_constructor") (result i32)
                (call $call
                    (i32.const 2)   ;; Offset address of the called contract
                    (i32.const 3)   ;; Length of the called contract
                    (i32.const 5)   ;; Offset address of the function name
                    (i32.const 3)   ;; Length of the function name
                    (i32.const 8)   ;; Offset address of the function args
                    (i32.const 12)) ;; Length of the function args
            )

            ;; Called contract
            (data (i32.const 2) "two")

            ;; Function name
            (data (i32.const 5) "sum")

            ;; Function args
            (data (i32.const 8) "\01\00\00\00\00\00\00\00\00\00\00\02")
        )
        "#;

        let result = runner.run(wat, None, vec![]);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Value::I32(0));
    }
}
