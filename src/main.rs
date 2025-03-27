use std::path::{Path, PathBuf};
use std::{fs, vec};
use std::fs::File;
use std::io::{Seek, Read, SeekFrom};
use std::mem::size_of;
use std::collections::HashMap;
use regex::Regex;
use std::env;

const VECTOR_SIZE: usize = 12;
const I32_SIZE: usize = size_of::<i32>();
const U8_SIZE: usize = size_of::<u8>();
const U16_SIZE: usize = size_of::<u16>();
const OFFSET_TO_TEXTURE: usize = I32_SIZE * 17 + VECTOR_SIZE * 6 + U8_SIZE * 64;
const TEXTURE_SIZE: usize = 64;

fn read_exact_from_file(f: &mut File, start: u64, size: usize) -> Option<Vec<u8>> {
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = vec![0; size];
    f.read_exact(&mut buf).ok()?;
    Some(buf)
}

fn i32_from_slice(slice: &[u8]) -> Option<i32> {
    Some(i32::from_ne_bytes(slice.try_into().ok()?))
}

fn null_term_str(f: &mut File, ofs: u64) -> Option<String> {
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

fn parse_vmt(vmt_str: &str) -> Option<HashMap<String, String>> {
    let re = Regex::new(r#""([^"]+)"\s*"([^"]+)""#).ok()?;
    Some(
        re
            .captures_iter(vmt_str)
            .fold(HashMap::new(), |mut acc, caps| {
                acc.insert(caps[1].to_string().to_lowercase(), caps[2].to_string());
                acc
            })
    )
}

struct Texture
{
    name: String, // Offset for null-terminated string
}

struct VMTInfo {
    base_texture: String,
    detail: String,
    bumpmap: String,
}

struct TexturesInfo {
    name: String,
    dirs: Vec<String>,
    textures: Vec<String>,
}

impl Texture {
    fn new(f: &mut File, i: i32, tex_ofs: i32) -> Option<Self> {
        let offset = (tex_ofs + TEXTURE_SIZE as i32 * i) as u64;
        let texture_buf = read_exact_from_file(f, offset, TEXTURE_SIZE)?;
        let name_ofs_u8 = texture_buf.get(0..I32_SIZE)?;
        let name_offset = i32_from_slice(name_ofs_u8)?;

        Some(Self {
            name: null_term_str(f, offset + name_offset as u64)?,
        })
    }
}

impl VMTInfo {
    fn new(path_str: &str) -> Option<Self> {
        let path = Path::new(path_str);

        if !path.exists() || path.extension().unwrap() != "vmt" {
            return None;
        }

        let mut f = File::open(path).ok()?;
        let mut str_buf = String::from("");
        let _ = f.read_to_string(&mut str_buf);

        let vmt_data = parse_vmt(str_buf.trim())?;

        let empty_str_ptr = &String::from("");
        let basetexture_val = vmt_data.get("$basetexture").unwrap_or(empty_str_ptr);
        let detail_val = vmt_data.get("$detail").unwrap_or(empty_str_ptr);
        let bumpmap_val = vmt_data.get("$bumpmap").unwrap_or(empty_str_ptr);

        let base_texture = if basetexture_val.is_empty() { basetexture_val.to_string() } else { format!("{}.vtf", basetexture_val) };
        let detail = if detail_val.is_empty() { detail_val.to_string() } else { format!("{}.vtf", detail_val) };
        let bumpmap = if bumpmap_val.is_empty() { bumpmap_val.to_string() } else { format!("{}.vtf", bumpmap_val) };

        Some(Self {
            base_texture,
            detail,
            bumpmap,
        })
    }
}

impl TexturesInfo {
    fn new(path: &str) -> Option<Self> {
        let mut f = File::open(path).ok()?;
        let mut_ptr = &mut f;
        let texture_info_buf = read_exact_from_file(mut_ptr, OFFSET_TO_TEXTURE as u64, I32_SIZE * 4)?;
        let tex_count = i32_from_slice(texture_info_buf.get(0..I32_SIZE)?)?;
        let tex_ofs = i32_from_slice(texture_info_buf.get( I32_SIZE..(I32_SIZE * 2))?)?;
        let texdir_count   = i32_from_slice(texture_info_buf.get((I32_SIZE * 2)..(I32_SIZE * 3))?)?;
        let texdir_ofs  = i32_from_slice(texture_info_buf.get((I32_SIZE * 3)..(I32_SIZE * 4))?)?;

        let dirs = (0..texdir_count).filter_map(|i| {
            mut_ptr.seek(SeekFrom::Start((texdir_ofs + (2 * U16_SIZE as i32) * i) as u64)).ok()?;
            let mut u16_bytes: [u8; 2] = [0; 2];
            let _ = mut_ptr.read(&mut u16_bytes).ok()?;
            let new_ofs = u16::from_le_bytes(u16_bytes);
            null_term_str(mut_ptr, new_ofs as u64)
        }).collect();

        let textures = (0..tex_count).filter_map(|i| {
            Some(Texture::new(mut_ptr, i, tex_ofs)?.name)
        }).collect();

        let name_u8_vec = read_exact_from_file(mut_ptr, (I32_SIZE * 3) as u64, 64)?
            .into_iter()
            .filter(|&el| el != 0)
            .collect();


        Some(Self {
            name: String::from_utf8(name_u8_vec).ok()?,
            dirs, 
            textures 
        })
    }
    
    fn download(&self, find_path_str: &str, output: &str) {
        let find_path = Path::new(find_path_str);
        let find_mdl_path = find_path.join("models");
        let find_mat_path = find_path.join("materials");

        let output_path = Path::new(output);
        let output_mdls_path = output_path.join("models");
        let output_mats_path = output_path.join("materials");

        let self_name_path = Path::new(&self.name);
        let self_name_parent = match self_name_path.parent() {
            Some(parent) => parent,
            None => {
                eprintln!("Не удалось получить родительскую директорию!");
                return;
            }
        };
        let self_file_stem = match self_name_path.file_stem() {
            Some(r) => r,
            None => {
                eprintln!("Не удалось stem файла!");
                return;
            }
        };

        let find_mdl_dir = find_mdl_path.join(self_name_parent);
        let output_mdl_dir = output_mdls_path.join(self_name_parent);

        if let Err(err) = fs::create_dir_all(&output_mdl_dir) {
            eprintln!("{}", err);
            return;
        }
        if let Err(err) = fs::create_dir_all(&output_mats_path) {
            eprintln!("{}", err);
            return;
        }

        let _ = &self.dirs.iter().for_each(|tex_dir_str| {
            let tex_dir_path: PathBuf = find_mat_path.join(tex_dir_str);

            if !tex_dir_path.exists() {
                eprintln!("Директория не существует: {}", tex_dir_str);
                return;
            }

            let _ = &self.textures.iter().for_each(|vmt_tex_stem| {
                let vmt_tex_name = &format!("{}.vmt", vmt_tex_stem);
                let vmt_tex_file_path: PathBuf = tex_dir_path.join(vmt_tex_name);

                if !vmt_tex_file_path.exists() {
                    return;
                }

                let vmt_tbl_opt = match vmt_tex_file_path.to_str() {
                    Some(path) => VMTInfo::new(path),
                    None => {
                        eprintln!("Путь не может быть преобразован в строку: {}", vmt_tex_stem);
                        return;
                    },
                };

                let vmt_tbl = match vmt_tbl_opt {
                    Some(r) => r,
                    None => {
                        eprintln!("Не удалось получить VMTInfo: {}", vmt_tex_stem);
                        return;
                    }
                };
            
                [vmt_tbl.detail, vmt_tbl.bumpmap, vmt_tbl.base_texture].iter().for_each(|vtf_file_path_str| {
                    if vtf_file_path_str.is_empty() {
                        return;
                    }

                    let vtf_file_path = Path::new(vtf_file_path_str);
                    let vtf_input_file_path = find_mat_path.join(vtf_file_path);

                    if !vtf_input_file_path.exists() {
                        return;
                    }

                    let vtf_output_file_path = output_mats_path.join(vtf_file_path);
                    let vtf_output_parent_dir_path = match vtf_output_file_path.parent() {
                        Some(r) => r,
                        None => {
                            eprintln!("Не удалось получить parent output директории VTF файла: {}", vtf_file_path_str);
                            return;
                        }
                    };

                    let _ = fs::create_dir_all(vtf_output_parent_dir_path);
                    let _ = fs::copy(vtf_input_file_path, vtf_output_file_path);
                });

                let vmt_tex_new_dir_path = output_mats_path.join(tex_dir_str);
                let vmt_tex_new_file_path: PathBuf = vmt_tex_new_dir_path.join(vmt_tex_name);
                let _ = fs::create_dir_all(&vmt_tex_new_dir_path);
                let _ = fs::copy(vmt_tex_file_path, vmt_tex_new_file_path);
            });
        });

        let find_mdl_dir_content = match fs::read_dir(find_mdl_dir) {
            Ok(r) => r,
            Err(err) => {
                eprintln!("{}", err);
                return;
            }
        };

        find_mdl_dir_content
            .filter_map(|dir_entry| {
                let entry = match dir_entry {
                    Ok(r) => r,
                    Err(err) => {
                        eprintln!("{}", err);
                        return None;
                    },
                };
                let entry_path = entry.path();

                if !entry_path.is_file() {
                    return None;
                }

                let file_name = entry_path.file_name()?.to_str()?;
                if !file_name.contains(self_file_stem.to_str()?) {
                    return None;
                }
            
                Some(entry_path)
            })
            .for_each(|input_file_path_buf| {
                let file_name = match input_file_path_buf.file_name() {
                    Some(r) => r,
                    None => {
                        let path_str = match input_file_path_buf.to_str() {
                            Some(r) => r,
                            None => "?",
                        };
                        eprintln!("Не удалось получить OsStr file_name: {}", path_str);
                        return;
                    }
                };

                let file_name_str = match file_name.to_str() {
                    Some(r) => r,
                    None => {
                        let path_str = match input_file_path_buf.to_str() {
                            Some(r) => r,
                            None => "?",
                        };
                        eprintln!("Не преобразовать OsStr в str file_name: {}", path_str);
                        return;
                    }
                };

                let output_file_path = output_mdl_dir.join(file_name_str);
                let _ = fs::copy(input_file_path_buf, output_file_path);
            });
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: {} <find_path> <output_path> <mdl_file_path>", args[0]);
        std::process::exit(1);
    }

    let find_path = &args[1];
    let output_path = &args[2];
    let mdl_file_path = &args[3];

    let texture_info = match TexturesInfo::new(mdl_file_path) {
        Some(r) => r,
        None => {
            eprintln!("Не удалось создать TexturesInfo!");
            std::process::exit(1);
        }
    };
    texture_info.download(find_path, output_path);
}