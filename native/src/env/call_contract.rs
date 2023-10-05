use crate::{
    data_entry::DataEntry,
    env::Environment,
    env_items, env_runtime,
    jvm::Jvm,
    runtime::{Runtime, RuntimeError},
};
use convert_case::{Case, Casing};
use std::str;
use wasmi::{core::Value, Caller, Func, Store};

env_items!(
    CallArgInt,
    CallArgBool,
    CallArgBinary,
    CallArgString,
    CallPayment,
    CallContract
);

env_runtime! {
    #[version = 0]
    pub fn CallArgInt(value: i64) {
        |mut caller: Caller<Runtime>| {
            caller.data_mut().args.push(DataEntry::Integer(value));
        }
    }
}

env_runtime! {
    #[version = 0]
    pub fn CallArgBool(value: i32) {
        |mut caller: Caller<Runtime>| {
            caller.data_mut().args.push(DataEntry::Boolean(value));
        }
    }
}

env_runtime! {
    #[version = 0]
    pub fn CallArgBinary(offset_value: u32, length_value: u32) -> i32 {
        |mut caller: Caller<Runtime>| {
            let (memory, ctx) = match caller.data().memory() {
                Some(memory) => memory.data_and_store_mut(&mut caller),
                None => return RuntimeError::MemoryNotFound as i32,
            };

            let value = &memory[offset_value as usize..offset_value as usize + length_value as usize];
            ctx.args.push(DataEntry::Binary(value.to_vec()));

            0
        }
    }
}

env_runtime! {
    #[version = 0]
    pub fn CallArgString(offset_value: u32, length_value: u32) -> i32 {
        |mut caller: Caller<Runtime>| {
            let (memory, ctx) = match caller.data().memory() {
                Some(memory) => memory.data_and_store_mut(&mut caller),
                None => return RuntimeError::MemoryNotFound as i32,
            };

            let value = &memory[offset_value as usize..offset_value as usize + length_value as usize];
            ctx.args.push(DataEntry::String(value.to_vec()));

            0
        }
    }
}

env_runtime! {
    #[version = 0]
    pub fn CallPayment(offset_asset_id: u32, length_asset_id: u32, amount: i64) -> i32 {
        |mut caller: Caller<Runtime>| {
            let (memory, ctx) = match caller.data().memory() {
                Some(memory) => memory.data_and_store_mut(&mut caller),
                None => return RuntimeError::MemoryNotFound as i32,
            };

            let asset_id = &memory[offset_asset_id as usize..offset_asset_id as usize + length_asset_id as usize];
            ctx.payments.push(asset_id, amount);

            0
        }
    }
}

env_runtime! {
    #[version = 0]
    pub fn CallContract(
        offset_contract_id: u32,
        length_contract_id: u32,
        offset_func_name: u32,
        length_func_name: u32,
    ) -> i32 {
        |mut caller: Caller<Runtime>| {
            let (memory, ctx) = match caller.data().memory() {
                Some(memory) => memory.data_and_store_mut(&mut caller),
                None => return RuntimeError::MemoryNotFound as i32,
            };

            let contract_id = &memory[offset_contract_id as usize..offset_contract_id as usize + length_contract_id as usize];

            let bytecode = match ctx.stack.get_bytecode(contract_id) {
                Ok(bytecode) => bytecode,
                Err(error) => return error.as_i32(),
            };

            let func_name = match str::from_utf8(
                &memory[offset_func_name as usize..offset_func_name as usize + length_func_name as usize]
            ) {
                Ok(string) => string,
                Err(_) => return RuntimeError::Utf8Error as i32,
            };

            let (input_data, payments) = ctx.args_and_payments();

            match ctx.stack.add_payments(contract_id, &payments) {
                Ok(()) => (),
                Err(error) => return error.as_i32(),
            }

            match ctx.stack.call(contract_id.to_vec(), bytecode, func_name, input_data) {
                Ok(result) => {
                    // TODO: Functions cannot return any values, they can only return an error code
                    if result.len() != 1 {
                        return RuntimeError::InvalidResult as i32;
                    }

                    match result[0] {
                        Value::I32(value) => value,
                        _ => RuntimeError::InvalidResult as i32,
                    }
                },
                Err(error) => error.as_i32(),
            }
        }
    }
}