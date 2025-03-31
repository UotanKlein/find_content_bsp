pub mod utils {
    use std::fs::File;
    use std::io::{Seek, Read, SeekFrom};
    pub fn read_exact_from_file(f: &mut File, start: u64, size: usize) -> Option<Vec<u8>> {
        f.seek(SeekFrom::Start(start)).ok()?;
        let mut buf = vec![0; size];
        f.read_exact(&mut buf).ok()?;
        Some(buf)
    }

    pub fn u16_from_slice(slice: &[u8]) -> Option<u16> {
        Some(u16::from_le_bytes(slice.try_into().ok()?))
    }

    pub fn i16_from_slice(slice: &[u8]) -> Option<i16> {
        Some(i16::from_le_bytes(slice.try_into().ok()?))
    }

    pub fn i32_from_slice(slice: &[u8]) -> Option<i32> {
        Some(i32::from_le_bytes(slice.try_into().ok()?))
    }

    pub fn f32_from_slice(slice: &[u8]) -> Option<f32> {
        Some(f32::from_le_bytes(slice.try_into().ok()?))
    }

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