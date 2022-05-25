#[derive(Debug, Clone)]
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

pub trait Sized {
    fn size(&self) -> usize;
}

pub trait Aligned {
    fn align(&self) -> usize;
}

impl Sized for PType {
    fn size(&self) -> usize {
        match self {
            PType::Bool => 1,
            PType::I32 => 4,
            PType::I64 => 8,
            PType::U32 => 4,
            PType::U64 => 8,
            PType::F16 => 2,
            PType::F32 => 4,
            PType::F64 => 8,
        }
    }
}

impl Aligned for PType {
    fn align(&self) -> usize {
        match self {
            PType::F16 => 2,
            PType::I32 => 4,
            PType::U32 => 4,
            PType::F32 => 4,
            PType::Bool => unreachable!(),
            PType::I64 => unreachable!(),
            PType::U64 => unreachable!(),
            PType::F64 => unreachable!(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum TType {
    Scalar(PType),
    Vector(usize, PType),
    Matrix { m: usize, n: usize, typed: PType },
    Array(usize, Box<TType>),
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
            Matrix { n, typed: x, .. } => Vector(*n, x.clone()).align(),
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

#[derive(Debug)]
pub struct StructSlot {
    pub identifier: String,
    pub typed: TType,
    pub comment: String,
}

#[derive(Debug)]
pub struct DynamicStruct {
    pub slots: Vec<StructSlot>,
    buffer: Vec<u8>,
}

impl DynamicStruct {
    pub fn new(slots: Vec<StructSlot>) -> Self {
        let mut buffer = Vec::with_capacity(slots.size());
        buffer.resize_with(slots.size(), Default::default);
        Self { slots, buffer }
    }

    pub fn write_to_slot<T: bytemuck::Pod>(&mut self, slot: usize, data: &T) -> () {
        let data_bufer = bytemuck::bytes_of(data);
        let slot_offset = offset_of_member(&self.slots, slot);
        for i in 0..data_bufer.len() {
            self.buffer[i + slot_offset] = data_bufer[i];
        }
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
}

impl Sized for Vec<StructSlot> {
    fn size(&self) -> usize {
        let n = offset_of_member(&self, self.len()) + self.last().unwrap().typed.size();
        round_up(self.align(), n)
    }
}

impl Aligned for Vec<StructSlot> {
    fn align(&self) -> usize {
        self.iter().map(|s| s.typed.align()).max().unwrap()
    }
}

mod test {
    use crate::wgsl::{Aligned, PType, Sized, TType};

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
            (PType::Bool, 1),
            (PType::I32, 4),
            (PType::I64, 8),
            (PType::U32, 4),
            (PType::U64, 8),
            (PType::F16, 2),
            (PType::F32, 4),
            (PType::F64, 8),
        ];

        for (ptype, size) in ptype_and_size {
            assert!(ptype.size() == size);
        }
    }

    #[test]
    fn align_with_valid_ptype() {
        let ptype_and_align = vec![
            (PType::F16, 2),
            (PType::I32, 4),
            (PType::U32, 4),
            (PType::F32, 4),
        ];

        for (ptype, align) in ptype_and_align {
            assert!(ptype.align() == align);
        }
    }

    #[test]
    fn align_with_invalid_ptype() {
        let ptypes = vec![PType::Bool, PType::I64, PType::U64, PType::F64];

        for ptype in ptypes {
            let result = catch_unwind_silent(|| ptype.align());
            assert!(result.is_err());
        }
    }

    #[test]
    fn align_with_valid_scalar_ttype() {
        let ptype_and_align = vec![
            (PType::F16, 2),
            (PType::I32, 4),
            (PType::U32, 4),
            (PType::F32, 4),
        ];

        for (ptype, align) in ptype_and_align {
            assert!(TType::Scalar(ptype).align() == align);
        }
    }

    #[test]
    fn align_with_valid_vector_f16_ttype() {
        let vec_size_and_align = vec![(2, 4), (3, 8), (4, 8)];

        for (vec_size, align) in vec_size_and_align {
            assert!(TType::Vector(vec_size, PType::F16).align() == align);
        }
    }

    #[test]
    fn align_with_valid_vector_4bytes_ttype() {
        let vec_size_and_align = vec![(2, 8), (3, 16), (4, 16)];

        for (vec_size, align) in vec_size_and_align.iter() {
            assert!(TType::Vector(*vec_size, PType::F32).align() == *align);
        }

        for (vec_size, align) in vec_size_and_align.iter() {
            assert!(TType::Vector(*vec_size, PType::I32).align() == *align);
        }

        for (vec_size, align) in vec_size_and_align.iter() {
            assert!(TType::Vector(*vec_size, PType::U32).align() == *align);
        }
    }

    #[test]
    fn align_with_valid_matrix_f16_ttype() {
        let matrix_size_and_align = vec![(2, 4), (3, 8), (4, 8)];

        for (matrix_size, align) in matrix_size_and_align {
            for m in [2, 3, 4] {
                assert!(
                    TType::Matrix {
                        m,
                        n: matrix_size,
                        typed: PType::F16
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
                    TType::Matrix {
                        m,
                        n: *matrix_size,
                        typed: PType::F32
                    }
                    .align()
                        == *align
                );
            }
        }

        for (matrix_size, align) in matrix_size_and_align.iter() {
            for m in [2, 3, 4] {
                assert!(
                    TType::Matrix {
                        m,
                        n: *matrix_size,
                        typed: PType::I32
                    }
                    .align()
                        == *align
                );
            }
        }

        for (matrix_size, align) in matrix_size_and_align.iter() {
            for m in [2, 3, 4] {
                assert!(
                    TType::Matrix {
                        m,
                        n: *matrix_size,
                        typed: PType::U32
                    }
                    .align()
                        == *align
                );
            }
        }
    }
}