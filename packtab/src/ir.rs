/// Integer types for generated code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntType {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
}

impl IntType {
    /// Bit width of this type.
    pub fn bits(self) -> u8 {
        match self {
            IntType::U8 | IntType::I8 => 8,
            IntType::U16 | IntType::I16 => 16,
            IntType::U32 | IntType::I32 => 32,
            IntType::U64 | IntType::I64 => 64,
        }
    }

    /// Whether this is an unsigned type.
    pub fn is_unsigned(self) -> bool {
        matches!(self, IntType::U8 | IntType::U16 | IntType::U32 | IntType::U64)
    }

    /// The smallest type that fits [min_v, max_v].
    pub fn for_range(min_v: i64, max_v: i64) -> IntType {
        if 0 <= min_v && max_v <= 255 {
            return IntType::U8;
        }
        if -128 <= min_v && max_v <= 127 {
            return IntType::I8;
        }
        if 0 <= min_v && max_v <= 65535 {
            return IntType::U16;
        }
        if -32768 <= min_v && max_v <= 32767 {
            return IntType::I16;
        }
        if 0 <= min_v && max_v <= 4_294_967_295 {
            return IntType::U32;
        }
        if -2_147_483_648 <= min_v && max_v <= 2_147_483_647 {
            return IntType::I32;
        }
        if 0 <= min_v {
            return IntType::U64;
        }
        IntType::I64
    }

    /// Short name like "u8", "i32".
    pub fn abbr(self) -> &'static str {
        match self {
            IntType::U8 => "u8",
            IntType::U16 => "u16",
            IntType::U32 => "u32",
            IntType::U64 => "u64",
            IntType::I8 => "i8",
            IntType::I16 => "i16",
            IntType::I32 => "i32",
            IntType::I64 => "i64",
        }
    }
}

/// A declared array in the generated code.
#[derive(Debug, Clone)]
pub struct ArrayDecl {
    pub name: String,
    pub typ: IntType,
    pub values: Vec<i64>,
    pub private: bool,
}

/// A declared function in the generated code.
#[derive(Debug, Clone)]
pub struct FuncDecl {
    pub name: String,
    pub ret_type: IntType,
    pub arg_name: String,
    pub body: String,
    pub private: bool,
    pub inline_always: bool,
}

/// Sub-byte accessor function declaration.
#[derive(Debug, Clone)]
pub struct AccessorDecl {
    pub name: String,
    pub unit_bits: u8,
    pub inline_always: bool,
}

/// The complete generated code: arrays + functions.
#[derive(Debug, Clone)]
pub struct CodeIR {
    pub arrays: Vec<ArrayDecl>,
    pub accessors: Vec<AccessorDecl>,
    pub functions: Vec<FuncDecl>,
}

impl CodeIR {
    pub fn new() -> Self {
        Self {
            arrays: Vec::new(),
            accessors: Vec::new(),
            functions: Vec::new(),
        }
    }
}
