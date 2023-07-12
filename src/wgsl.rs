use std::ops::RangeInclusive;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PType {
    Bool,
    I32,
    I64,
    U32,
    U64,
    F16,
    F32,
    F64,
}

impl From<&PType> for String {
    fn from(ptype: &PType) -> Self {
        use PType::*;
        match ptype {
            Bool => "bool",
            I32 => "i32",
            I64 => "i64",
            U32 => "u32",
            U64 => "u64",
            F16 => "f16",
            F32 => "f32",
            F64 => "f64",
        }
        .to_owned()
    }
}

pub trait Sized {
    fn size(&self) -> usize;
}

pub trait Aligned {
    fn align(&self) -> usize;
}

impl Sized for PType {
    fn size(&self) -> usize {
        use PType::*;
        match self {
            Bool => 1,
            F16 => 2,
            I32 | U32 | F32 => 4,
            I64 | U64 | F64 => 8,
        }
    }
}

impl Aligned for PType {
    fn align(&self) -> usize {
        use PType::*;
        match self {
            F16 => 2,
            I32 | U32 | F32 => 4,
            Bool => unreachable!(),
            I64 => unreachable!(),
            U64 => unreachable!(),
            F64 => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TType {
    Scalar(PType),
    Vector(usize, PType),
    Matrix { m: usize, n: usize, typed: PType },
    Array(usize, Box<TType>),
}

impl From<&TType> for String {
    fn from(ttype: &TType) -> Self {
        use TType::*;
        match ttype {
            Scalar(x) => x.into(),
            Vector(n, x) => format!("vec{}<{}>", n, String::from(x)),
            Matrix { m, n, typed } => format!("mat{}x{}<{}>", m, n, String::from(typed)),
            Array(n, x) => format!("array<{},{}>", String::from(&**x), n),
        }
    }
}

impl Sized for TType {
    fn size(&self) -> usize {
        use TType::*;
        match self {
            Scalar(x) => x.size(),
            Vector(nb, x) => *nb as usize * x.size(),
            Matrix { m, n, typed: x } => m * n * x.size(),
            Array(n, x) => n * x.size(),
        }
    }
}

impl Aligned for TType {
    fn align(&self) -> usize {
        use TType::*;
        match self {
            Scalar(x) => x.align(),
            Vector(nb, x) => (*nb as usize * x.align()).next_power_of_two(),
            Matrix { n, typed: x, .. } => Vector(*n, *x).align(),
            Array(_n, x) => x.align(),
        }
    }
}

fn round_up(k: usize, n: usize) -> usize {
    (n as f64 / k as f64).ceil() as usize * k
}

fn offset_of_member(struc: &Vec<StructSlot>, slot: usize) -> usize {
    if slot == 1 {
        return 0;
    }
    let member = &struc.get(slot - 1).unwrap().typed;
    let prev_member = &struc.get(slot - 2).unwrap().typed;
    let k = member.align();
    let n = offset_of_member(struc, slot - 1) + prev_member.size();
    round_up(k, n)
}

#[derive(Debug, PartialEq)]
pub enum StructSlotOptions {
    Slider { range: RangeInclusive<f32> },
}

#[derive(Debug, PartialEq)]
pub struct StructSlot {
    pub identifier: String,
    pub typed: TType,
    pub options: Option<StructSlotOptions>,
}

impl StructSlot {
    pub fn generate_definition(&self) -> String {
        format!(
            "fn {ident}() -> {typed} {{return _gui.{ident};}}",
            ident = self.identifier,
            typed = String::from(&self.typed)
        )
    }
}

#[derive(Debug)]
pub struct RuntimeStruct {
    pub slots: Vec<StructSlot>,
    buffer: Vec<u8>,
}

impl RuntimeStruct {
    pub fn new(slots: Vec<StructSlot>) -> Self {
        let mut buffer = Vec::with_capacity(slots.size());
        buffer.resize_with(slots.size(), Default::default);
        Self { slots, buffer }
    }

    pub fn write_to_slot<T: bytemuck::Pod>(&mut self, slot: usize, data: &T) {
        let data_bufer = bytemuck::bytes_of(data);
        let slot_offset = offset_of_member(&self.slots, slot);
        self.buffer[slot_offset..data_bufer.len() + slot_offset].copy_from_slice(data_bufer);
    }

    pub fn get_slot_number(&self, slot_name: &str) -> Option<usize> {
        self.slots
            .iter()
            .enumerate()
            .find(|s| s.1.identifier == slot_name)
            .map(|s| s.0 + 1)
    }

    pub fn read_from_slot_ref_mut<T: bytemuck::Pod>(&mut self, slot: usize) -> &mut T {
        let size_to_read = std::mem::size_of::<T>();
        let slot_offset = offset_of_member(&self.slots, slot);
        bytemuck::from_bytes_mut(&mut self.buffer[slot_offset..slot_offset + size_to_read])
    }

    pub fn buffer(&self) -> &[u8] {
        &self.buffer[..]
    }

    pub fn buffer_mut(&mut self) -> &mut [u8] {
        &mut self.buffer[..]
    }
}

impl Sized for Vec<StructSlot> {
    fn size(&self) -> usize {
        let n = offset_of_member(self, self.len()) + self.last().unwrap().typed.size();
        round_up(self.align(), n)
    }
}

impl Aligned for Vec<StructSlot> {
    fn align(&self) -> usize {
        self.iter().map(|s| s.typed.align()).max().unwrap()
    }
}

#[allow(unused)]
mod tests {
    use crate::wgsl::{Aligned, PType, Sized, TType};
    use PType::*;
    use TType::*;

    fn catch_unwind_silent<F: FnOnce() -> R + std::panic::UnwindSafe, R>(
        f: F,
    ) -> std::thread::Result<R> {
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let result = std::panic::catch_unwind(f);
        std::panic::set_hook(prev_hook);
        result
    }

    #[test]
    fn correctly_sized_ptype() {
        let ptype_and_size = vec![
            (Bool, 1),
            (I32, 4),
            (I64, 8),
            (U32, 4),
            (U64, 8),
            (F16, 2),
            (F32, 4),
            (F64, 8),
        ];

        for (ptype, size) in ptype_and_size {
            assert!(ptype.size() == size);
        }
    }

    #[test]
    fn align_with_valid_ptype() {
        let ptype_and_align = vec![(F16, 2), (I32, 4), (U32, 4), (F32, 4)];

        for (ptype, align) in ptype_and_align {
            assert!(ptype.align() == align);
        }
    }

    #[test]
    fn align_with_invalid_ptype() {
        let ptypes = vec![Bool, I64, U64, F64];

        for ptype in ptypes {
            let result = catch_unwind_silent(|| ptype.align());
            assert!(result.is_err());
        }
    }

    #[test]
    fn align_with_valid_scalar_ttype() {
        let ptype_and_align = vec![(F16, 2), (I32, 4), (U32, 4), (F32, 4)];

        for (ptype, align) in ptype_and_align {
            assert!(TType::Scalar(ptype).align() == align);
        }
    }

    #[test]
    fn align_with_valid_vector_f16_ttype() {
        let vec_size_and_align = vec![(2, 4), (3, 8), (4, 8)];

        for (vec_size, align) in vec_size_and_align {
            assert!(Vector(vec_size, F16).align() == align);
        }
    }

    #[test]
    fn align_with_valid_vector_4bytes_ttype() {
        let vec_size_and_align = vec![(2, 8), (3, 16), (4, 16)];

        for (vec_size, align) in vec_size_and_align.iter() {
            assert!(Vector(*vec_size, F32).align() == *align);
        }

        for (vec_size, align) in vec_size_and_align.iter() {
            assert!(Vector(*vec_size, I32).align() == *align);
        }

        for (vec_size, align) in vec_size_and_align.iter() {
            assert!(Vector(*vec_size, U32).align() == *align);
        }
    }

    #[test]
    fn align_with_valid_matrix_f16_ttype() {
        let matrix_size_and_align = vec![(2, 4), (3, 8), (4, 8)];

        for (matrix_size, align) in matrix_size_and_align {
            for m in [2, 3, 4] {
                assert!(
                    Matrix {
                        m,
                        n: matrix_size,
                        typed: F16
                    }
                    .align()
                        == align
                );
            }
        }
    }

    #[test]
    fn align_with_valid_matrix_4bytes_ttype() {
        let matrix_size_and_align = vec![(2, 8), (3, 16), (4, 16)];

        for (matrix_size, align) in matrix_size_and_align.iter() {
            for m in [2, 3, 4] {
                assert!(
                    Matrix {
                        m,
                        n: *matrix_size,
                        typed: F32
                    }
                    .align()
                        == *align
                );
            }
        }

        for (matrix_size, align) in matrix_size_and_align.iter() {
            for m in [2, 3, 4] {
                assert!(
                    Matrix {
                        m,
                        n: *matrix_size,
                        typed: I32
                    }
                    .align()
                        == *align
                );
            }
        }

        for (matrix_size, align) in matrix_size_and_align.iter() {
            for m in [2, 3, 4] {
                assert!(
                    Matrix {
                        m,
                        n: *matrix_size,
                        typed: U32
                    }
                    .align()
                        == *align
                );
            }
        }
    }
}
