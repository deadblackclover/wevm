use crate::{
    error::{Error, ExecutableError, Result},
    vm::Vm,
};
use jni::{InitArgsBuilder, JNIVersion, JavaVM};
use wasmi::Value;
use wevm_proc_macro::module;

/// Converts the given `.wat` into `.wasm`.
pub fn wat2wasm(wat: &str) -> Result<Vec<u8>, wat::Error> {
    wat::parse_str(wat)
}

#[module(env0)]
mod test {
    fn test_set_value(value: u32) {
        |mut _caller: Caller<Runtime>| {
            assert_eq!(42, value);
        }
    }

    fn test_get_value() -> u32 {
        |mut _caller: Caller<Runtime>| 42
    }

    fn test_memory(offset: u32, length: u32) {
        |mut caller: Caller<Runtime>| {
            let (memory, _ctx) = caller
                .data()
                .memory()
                .expect("Error get memory")
                .data_and_store_mut(&mut caller);

            let result =
                std::str::from_utf8(&memory[offset as usize..offset as usize + length as usize])
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

    pub fn run(&self, wat: &str, params: Vec<u8>) -> Result<Vec<Value>> {
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
        let memory: (u32, u32) = (1, 1);
        let fuel_limit = 1024;

        let mut stack = Vm::new(
            vec![],
            bytecode,
            memory,
            fuel_limit,
            modules::modules(),
            Some(jvm),
            Some(global_ref),
        )
        .expect("Call stack creation failed");

        stack.run("_constructor", &params)
    }
}

#[test]
fn test_vm() {
    let runner = TestRunner::new();

    // Base test
    {
        let wat = r#"
        (module
            (func (export "_constructor") (result i32)
                (i32.add
                    (i32.const 2)
                    (i32.const 2))
            )

            (global $__heap_base (export "__heap_base") i32 (i32.const 0))
        )
        "#;

        let result = runner.run(wat, vec![]);
        assert!(result.is_ok());

        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].i32(), Some(4));
    }

    // Import test
    {
        let wat = r#"
        (module
            (import "env0" "test_set_value" (func $test_set_value (param i32)))
            (import "env0" "test_get_value" (func $test_get_value (result i32)))

            (func (export "_constructor") (result i32)
                (call $test_set_value
                    (call $test_get_value))

                (i32.const 0)
            )

            (global $__heap_base (export "__heap_base") i32 (i32.const 0))
        )
        "#;

        let result = runner.run(wat, vec![]);
        assert!(result.is_ok());

        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].i32(), Some(0));
    }

    // Memory test
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

            (global $__heap_base (export "__heap_base") i32 (i32.const 0))

            ;; For test memory
            (data (i32.const 0) "Hi")
        )
        "#;

        let result = runner.run(wat, vec![]);
        assert!(result.is_ok());

        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].i32(), Some(0));
    }

    // Args test
    {
        let wat = r#"
        (module
            (func (export "_constructor") (param $p0 i32) (result i32)
                (i32.add
                    (local.get $p0)
                    (i32.const 2)
                )
            )

            (global $__heap_base (export "__heap_base") i32 (i32.const 0))
        )
        "#;

        let result = runner.run(
            wat,
            vec![
                0, 1, 0, 8, 116, 101, 115, 116, 95, 107, 101, 121, 0, 0, 0, 0, 0, 0, 0, 0, 1,
            ],
        );
        assert!(result.is_ok());

        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].i32(), Some(3));
    }

    // Negative test
    {
        let wat = r#"
        (module
            (func (export "_constructor") (param $p0 f32) (result f32)
                (f32.add
                    (local.get $p0)
                    (f32.const 2)
                )
            )

            (global $__heap_base (export "__heap_base") i32 (i32.const 0))
        )
        "#;

        let result = runner.run(
            wat,
            vec![
                0, 1, 0, 8, 116, 101, 115, 116, 95, 107, 101, 121, 0, 0, 0, 0, 0, 0, 0, 0, 1,
            ],
        );

        if let Err(Error::Executable(ExecutableError::InvalidBytecode(_))) = result {
            assert!(true);
        } else {
            assert!(false);
        }
    }
}
