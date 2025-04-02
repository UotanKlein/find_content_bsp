pub mod utils {
    use std::{fs::File, io::{Seek, Read, SeekFrom}};

    pub fn read_exact_from_file(f: &mut File, start: u64, size: usize) -> Option<Vec<u8>> {
        f.seek(SeekFrom::Start(start)).ok()?;
        let mut buf = vec![0; size];
        f.read_exact(&mut buf).ok()?;
        Some(buf)
    }

    pub fn read_segments_from_file(f: &mut File, start: u64, size_vec: &[usize]) -> Option<Vec<Vec<u8>>> {
        f.seek(SeekFrom::Start(start)).ok()?;
        size_vec.iter().map(|&size| {
            let mut buf = vec![0; size];
            f.read_exact(&mut buf).ok().map(|_| buf)
        }).collect()
    }

    pub trait FromSlice: Sized {
        fn from_u8_slice(slice: &[u8]) -> Option<Self>;
    }

    macro_rules! impl_from_slice {
        ($($t:ty),*) => {
            $(
                impl FromSlice for $t {
                    fn from_u8_slice(slice: &[u8]) -> Option<Self> {
                        Some(<$t>::from_le_bytes(slice.try_into().ok()?))
                    }
                }
            )*
        };
    }
    
    impl_from_slice!(u16, i16, i32, f32);

    pub fn null_term_str(f: &mut File, ofs: u64) -> Option<String> {
        f.seek(SeekFrom::Start(ofs)).ok();
        let mut buf = Vec::new();

        loop {
            let mut byte = [0u8; 1];
            let count_readed = f.read(&mut byte).ok()?;
            if count_readed == 0 || byte[0] == 0 {
                break;
            }
            buf.push(byte[0]);
        }

        Some(String::from_utf8(buf).ok()?)
    }
}