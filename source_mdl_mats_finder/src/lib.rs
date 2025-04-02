pub mod finder {
    use std::path::{Path, PathBuf};
    use std::{fs, vec};
    use std::fs::File;
    use std::io::{Seek, Read, SeekFrom};
    use std::mem::size_of;
    use std::collections::HashMap;
    use regex::Regex;
    use simple_utils::utils::{FromSlice, read_exact_from_file, read_segments_from_file, null_term_str };

    const VECTOR_SIZE: usize = 12;
    const TEX_SIZE: usize = 64;
    const OFS_TO_TEX: usize = I32_SIZE * 17 + VECTOR_SIZE * 6 + U8_SIZE * 64;
    const I32_SIZE: usize = size_of::<i32>();
    const U8_SIZE: usize = size_of::<u8>();
    const U16_SIZE: usize = size_of::<u16>();
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

    #[derive(Debug)]
    pub struct Texture {
        name: String,
    }

    impl Texture {
        pub fn new(f: &mut File, i: i32, tex_ofs: i32) -> Option<Self> {
            let ofs = (tex_ofs + TEX_SIZE as i32 * i) as u64;
            let tex_buf = read_exact_from_file(f, ofs, TEX_SIZE)?;
            let name_ofs = i32::from_u8_slice(tex_buf.get(0..I32_SIZE)?)?;
            Some(Self {
                name: null_term_str(f, ofs + name_ofs as u64)?,
            })
        }
    }

    #[derive(Debug)]
    pub struct VMTInfo {
        path: String,
        rel_path: String,
        vmt_data: HashMap<String, String>,
    }

    impl VMTInfo {
        pub fn new(vmt_tex_rel_path: &Path, find_mats_path: &Path) -> Option<Self> {
            let vmt_abs_path = &find_mats_path.join(vmt_tex_rel_path);

            if !vmt_abs_path.exists() || vmt_abs_path.extension()? != "vmt" {
                return None;
            }

            let mut f = File::open(vmt_abs_path).ok()?;
            let mut str_buf = String::from("");
            let _ = f.read_to_string(&mut str_buf);

            Some(Self {
                path: String::from(vmt_abs_path.to_str()?),
                rel_path: String::from(vmt_tex_rel_path.to_str()?),
                vmt_data: parse_vmt(str_buf.trim())?,
            })
        }
        
        pub fn download_vmt(&self, output_mats_path: &Path) {
            let vmt_path_str = &self.path;
            let vmt_rel_path_str = &self.rel_path;
            let vmt_path = Path::new(vmt_path_str);
            let vmt_rel_path = Path::new(vmt_rel_path_str);
            let vmt_tex_new_file_path = output_mats_path.join(vmt_rel_path);
            let vmt_tex_new_dir_path = match vmt_tex_new_file_path.parent() {
                Some(r) => r,
                None => {
                    return;
                },
            };

            let _ = fs::create_dir_all(vmt_tex_new_dir_path);
            let _ = fs::copy(vmt_path, vmt_tex_new_file_path);
        }

        pub fn download_vtf(&self, find_mats_path: &Path, output_mats_path: &Path, keys: &Vec<&str>) {
            let vmt_data_ptr = &self.vmt_data;
            let empty_str_ptr = &String::from("");

            keys.iter().for_each(|&vmt_key| {
                let vtf_val = vmt_data_ptr.get(vmt_key).unwrap_or(empty_str_ptr);

                if vtf_val.is_empty() {
                    return;
                }

                let vtf_file_path_str = format!("{}.vtf", vtf_val);
                let vtf_file_path = Path::new(&vtf_file_path_str);
                let vtf_input_file_path = find_mats_path.join(vtf_file_path);

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

            match vmt_data_ptr.get("include") {
                Some(include_path_str) => {
                    let include_path: PathBuf = Path::new(&include_path_str).iter().skip(1).collect();
                    match Self::new(&include_path, find_mats_path) {
                        Some(vmt_info) => {
                            vmt_info.download_with_def_keys(find_mats_path, output_mats_path);
                        },
                        None => {},
                    }
                },
                None => {},
            };
        }

        pub fn download(&self, find_mats_path: &Path, output_mats_path: &Path, keys: &Vec<&str>) {
            self.download_vmt(output_mats_path);
            self.download_vtf(find_mats_path, output_mats_path, keys);
        }

        pub fn download_with_def_keys(&self, find_mats_path: &Path, output_mats_path: &Path) {
            let vmt_keys = vec!["$basetexture", "$detail", "$bumpmap", "$envmapmask", "$selfillummask"];
            self.download(find_mats_path, output_mats_path, &vmt_keys);
        }
    }

    #[derive(Debug)]
    pub struct TexturesInfo {
        name: String,
        dirs: Vec<String>,
        textures: Vec<String>,
    }

    impl TexturesInfo {
        pub fn new(path: &Path) -> Option<Self> {
            let mut f = match File::open(path) {
                Ok(r) => r,
                Err(err) => {
                    eprintln!("1: {} {}", err, path.to_str().unwrap());
                    return None;
                }
            };
            let mut_ptr = &mut f;
            let size_vec = vec![I32_SIZE, I32_SIZE, I32_SIZE, I32_SIZE];
            let tex_info_segments = read_segments_from_file(mut_ptr, OFS_TO_TEX as u64, &size_vec)?;
            let tex_count = i32::from_u8_slice(&tex_info_segments[0])?;
            let tex_ofs = i32::from_u8_slice(&tex_info_segments[1])?;
            let texdir_count   = i32::from_u8_slice(&tex_info_segments[2])?;
            let texdir_ofs  = i32::from_u8_slice(&tex_info_segments[3])?;
            let dirs = (0..texdir_count).filter_map(|i| {
                mut_ptr.seek(SeekFrom::Start((texdir_ofs + (2 * U16_SIZE as i32) * i) as u64)).ok()?;
                let mut u16_bytes: [u8; 2] = [0; 2];
                let _ = mut_ptr.read_exact(&mut u16_bytes).ok()?;
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
        
        pub fn download(&self, find_path: &Path, output_path: &Path) {
            let find_mdl_path = find_path.join("models");
            let find_mats_path = find_path.join("materials");

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
                eprintln!("2: {}", err);
                return;
            }
            if let Err(err) = fs::create_dir_all(&output_mats_path) {
                eprintln!("3: {}", err);
                return;
            }

            let _ = &self.dirs.iter().for_each(|tex_dir_str| {
                let tex_dir_path: PathBuf = find_mats_path.join(tex_dir_str);
                if !tex_dir_path.exists() {
                    eprintln!("Директория не существует: {}", tex_dir_str);
                    return;
                }

                let _ = &self.textures.iter().for_each(|vmt_tex_stem| {
                    let vmt_tex_rel_path = Path::new(tex_dir_str).join(format!("{}.vmt", vmt_tex_stem));
                    let vmt_info = match VMTInfo::new(&vmt_tex_rel_path, &find_mats_path) {
                        Some(r) => r,
                        None => {
                            return;
                        },
                    };

                    vmt_info.download_with_def_keys(&find_mats_path, &output_mats_path);
                });
            });

            let find_mdl_dir_content = match fs::read_dir(find_mdl_dir) {
                Ok(r) => r,
                Err(err) => {
                    eprintln!("4: {}", err);
                    return;
                }
            };

            find_mdl_dir_content
                .filter_map(|dir_entry| {
                    let entry = match dir_entry {
                        Ok(r) => r,
                        Err(err) => {
                            eprintln!("5: {}", err);
                            return None;
                        },
                    };
                    let entry_path = entry.path();

                    if !entry_path.is_file() {
                        return None;
                    }

                    let file_name = entry_path.file_name()?.to_string_lossy().to_lowercase();
                    let self_stem = self_file_stem.to_string_lossy().to_lowercase();
                    
                    if !file_name.contains(&self_stem) {
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
}
