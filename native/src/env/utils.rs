use crate::{error::RuntimeError, runtime::Runtime};
use base58::{FromBase58, ToBase58};
use std::str;
use wasmi::Caller;

pub fn base58(
    offset_bytes: u32,
    length_bytes: u32,
    mut caller: Caller<Runtime>,
) -> (i32, u32, u32) {
    let (memory, ctx) = match caller.data().memory() {
        Some(memory) => memory.data_and_store_mut(&mut caller),
        None => return (RuntimeError::MemoryNotFound as i32, 0, 0),
    };
    let offset_memory = ctx.heap_base() as usize;

    let value = match str::from_utf8(
        &memory[offset_bytes as usize..offset_bytes as usize + length_bytes as usize],
    ) {
        Ok(string) => string,
        Err(_) => return (RuntimeError::Utf8Error as i32, 0, 0),
    };

    match value.from_base58() {
        Ok(result) => crate::env::write_memory(ctx, memory, offset_memory, result),
        Err(_) => (RuntimeError::Base58Error as i32, 0, 0),
    }
}

pub fn to_base58_string(
    offset_bytes: u32,
    length_bytes: u32,
    mut caller: Caller<Runtime>,
) -> (i32, u32, u32) {
    let (memory, ctx) = match caller.data().memory() {
        Some(memory) => memory.data_and_store_mut(&mut caller),
        None => return (RuntimeError::MemoryNotFound as i32, 0, 0),
    };
    let offset_memory = ctx.heap_base() as usize;

    let value = &memory[offset_bytes as usize..offset_bytes as usize + length_bytes as usize];

    let result = value.to_base58().as_bytes().to_vec();
    crate::env::write_memory(ctx, memory, offset_memory, result)
}

pub fn to_le_bytes(
    offset_bytes: u32,
    length_bytes: u32,
    mut caller: Caller<Runtime>,
) -> (i32, u32, u32) {
    let (memory, ctx) = match caller.data().memory() {
        Some(memory) => memory.data_and_store_mut(&mut caller),
        None => return (RuntimeError::MemoryNotFound as i32, 0, 0),
    };
    let offset_memory = ctx.heap_base() as usize;

    let mut result =
        memory[offset_bytes as usize..offset_bytes as usize + length_bytes as usize].to_vec();
    result.reverse();

    crate::env::write_memory(ctx, memory, offset_memory, result)
}

pub fn caller(mut caller: Caller<Runtime>) -> (i32, u32, u32) {
    let (memory, ctx) = match caller.data().memory() {
        Some(memory) => memory.data_and_store_mut(&mut caller),
        None => return (RuntimeError::MemoryNotFound as i32, 0, 0),
    };
    let offset_memory = ctx.heap_base() as usize;

    let result = ctx.vm.get_caller_current_frame();

    crate::env::write_memory(ctx, memory, offset_memory, result)
}
