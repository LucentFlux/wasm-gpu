use wasmtime_environ::Trap;

pub fn trap_to_u32(trap: Option<Trap>) -> u32 {
    let trap = match trap {
        None => return 0,
        Some(trap) => trap,
    };
    match trap {
        Trap::StackOverflow => 1,
        Trap::MemoryOutOfBounds => 2,
        Trap::HeapMisaligned => 3,
        Trap::TableOutOfBounds => 4,
        Trap::IndirectCallToNull => 5,
        Trap::BadSignature => 6,
        Trap::IntegerOverflow => 7,
        Trap::IntegerDivisionByZero => 8,
        Trap::BadConversionToInteger => 9,
        Trap::UnreachableCodeReached => 10,
        Trap::Interrupt => 11,
        Trap::AlwaysTrapAdapter => 12,
        Trap::OutOfFuel => 13,
        Trap::AtomicWaitNonSharedMemory => 14,
        _ => panic!("unsupported trap code: {:?}", trap),
    }
}

pub fn u32_to_trap(trap: u32) -> Option<Trap> {
    match trap {
        0 => None,
        1 => Some(Trap::StackOverflow),
        2 => Some(Trap::MemoryOutOfBounds),
        3 => Some(Trap::HeapMisaligned),
        4 => Some(Trap::TableOutOfBounds),
        5 => Some(Trap::IndirectCallToNull),
        6 => Some(Trap::BadSignature),
        7 => Some(Trap::IntegerOverflow),
        8 => Some(Trap::IntegerDivisionByZero),
        9 => Some(Trap::BadConversionToInteger),
        10 => Some(Trap::UnreachableCodeReached),
        11 => Some(Trap::Interrupt),
        12 => Some(Trap::AlwaysTrapAdapter),
        13 => Some(Trap::OutOfFuel),
        14 => Some(Trap::AtomicWaitNonSharedMemory),
        _ => panic!("unsupported trap code: {:?}", trap),
    }
}
