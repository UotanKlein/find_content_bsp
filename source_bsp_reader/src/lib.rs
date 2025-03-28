pub mod reader {
    use std::fs;
    use std::fs::{File};
    use std::path::{Path, PathBuf};
    use std::mem::size_of;
    use regex::Regex;
    use std::collections::{HashMap, HashSet};
    use simple_utils::utils::{u16_from_slice, i16_from_slice, f32_from_slice, i32_from_slice, read_exact_from_file};
    use source_mdl_mats_finder::finder::{TexturesInfo, VMTInfo};

    const I32_SIZE: usize = size_of::<i32>();
    const F32_SIZE: usize = size_of::<f32>();
    const U16_SIZE: usize = size_of::<u16>();
    const I16_SIZE: usize = size_of::<i16>();
    const HEADER_LUMPS: usize = 64;
    const PS_NAME_SIZE: usize = 128;

    struct LumpT {
        file_ofs: i32,
        file_len: i32,
        version: i32,
        four_cc: [u8; I32_SIZE],
    }

    #[derive(Debug)]
    pub struct Vector {
        x: f32,
        y: f32,
        z: f32,
    }

    impl Vector {
        fn from_u8_vec(u8_vec: &[u8]) -> Option<Self> {
            Some(Self {
                x: f32_from_slice(u8_vec.get(0..F32_SIZE)?)?,
                y: f32_from_slice(u8_vec.get(F32_SIZE..(F32_SIZE * 2))?)?,
                z: f32_from_slice(u8_vec.get((F32_SIZE * 2)..(F32_SIZE * 3))?)?,
            })
        }
    }

    #[derive(Debug)]
    pub struct DModelT {
        mins: Vector,
        maxs: Vector,
        origin: Vector,
        headnode: i32,
        firstface: i32,
        numfaces: i32,
    }    

    #[derive(Debug)]
    pub struct DGameLumpT {
        id: i32,
        flags: u16,
        version: u16,
        file_ofs: i32,
        file_len: i32,
    }

    pub struct DHeaderT {
        path: String,
        ident: [u8; I32_SIZE],
        version: i32,
        lumps: [LumpT; HEADER_LUMPS],
        map_revision: i32,
    }

    const HEADER_SIZE: usize = size_of::<DHeaderT>();
    const LUMP_SIZE: usize = size_of::<LumpT>();
    const DGAME_LUMP_SIZE: usize = size_of::<DGameLumpT>();
    const DMODEL_SIZE: usize = size_of::<DModelT>();
    const VECTOR_SIZE: usize = size_of::<Vector>();

    fn get_bytes_4(bytes: &[u8], start: usize) -> Option<[u8; I32_SIZE]> {
        bytes[start..start + I32_SIZE].try_into().ok()
    }

    impl LumpT {
        fn new(header_bytes: &[u8], lump_num: usize) -> Option<Self> {
            let offset = I32_SIZE * 2 + lump_num * LUMP_SIZE;
            Some(Self {
                file_ofs: i32_from_slice(header_bytes.get(offset..(offset + I32_SIZE))?)?,
                file_len: i32_from_slice(header_bytes.get((offset + I32_SIZE)..(offset + I32_SIZE * 2))?)?,
                version: i32_from_slice(header_bytes.get((offset + I32_SIZE * 2)..(offset + I32_SIZE * 3))?)?,
                four_cc: get_bytes_4(header_bytes, offset + I32_SIZE * 3)?,
            })
        }
    }

    impl DHeaderT {
        pub fn new(path: &Path) -> Option<Self> {
            let mut f = File::open(path).ok()?;
            let header_bytes = read_exact_from_file(&mut f, 0, HEADER_SIZE)?;
            let map_revision_ofs = I32_SIZE * 2 + LUMP_SIZE * HEADER_LUMPS;
            Some(Self {
                path: String::from(path.to_str()?),
                ident: get_bytes_4(&header_bytes, 0)?,
                version: i32_from_slice(header_bytes.get(0..I32_SIZE)?)?,
                map_revision: i32_from_slice(header_bytes.get(map_revision_ofs..(map_revision_ofs + I32_SIZE))?)?,
                lumps: std::array::from_fn(|i| {
                    match LumpT::new(&header_bytes, i) {
                        Some(r) => r,
                        None => {
                            LumpT {
                                file_ofs: 0,
                                file_len: 0,
                                version: 0,
                                four_cc: [0; I32_SIZE],
                            }
                        }
                    }
                }),
            })
        }

        pub fn get_lump(&self, lump_id: usize) -> Option<Vec<u8>> {
            let lump0_info = &self.lumps[lump_id];
            let mut f = File::open(&self.path).ok()?;
            let ofs = lump0_info.file_ofs;
            let len = lump0_info.file_len;
            Some(read_exact_from_file(&mut f, ofs as u64, len as usize)?)
        }

        pub fn get_lump_0(&self) -> Option<Vec<HashMap<String, String>>> {
            let lump0_vec = self.get_lump(0)?;
            let lump0_str = String::from_utf8(lump0_vec).ok()?;
            let re_braces = Regex::new(r"\{([^}]*)\}").ok()?;
            let re_props = Regex::new(r#""([^"]+)"\s*"([^"]+)""#).ok()?;
            Some(re_braces.captures_iter(&lump0_str).map(|caps| {
                re_props.captures_iter(&caps[1].to_string()).fold(HashMap::new(), |mut acc, caps| {
                    acc.insert(caps[1].to_string(), caps[2].to_string());
                    acc
                })
            }).collect())
        }

        pub fn get_lump_14(&self) -> Option<Vec<DModelT>> {
            let lump45_u8_vec = self.get_lump(14)?;
            let count = lump45_u8_vec.len() / DMODEL_SIZE;
            Some((0..count).filter_map(|i| {
                let data_ofs = DMODEL_SIZE * i;
                Some(DModelT {
                    mins: Vector::from_u8_vec(&lump45_u8_vec.get(data_ofs..(data_ofs + VECTOR_SIZE))?)?,
                    maxs: Vector::from_u8_vec(&lump45_u8_vec.get((data_ofs + VECTOR_SIZE)..(data_ofs + VECTOR_SIZE * 2))?)?,
                    origin: Vector::from_u8_vec(&lump45_u8_vec.get((data_ofs + VECTOR_SIZE * 2)..(data_ofs + VECTOR_SIZE * 3))?)?,
                    headnode: i32_from_slice(&lump45_u8_vec.get((data_ofs + VECTOR_SIZE * 3)..(data_ofs + VECTOR_SIZE * 3 + I32_SIZE))?)?,
                    firstface: i32_from_slice(&lump45_u8_vec.get((data_ofs + VECTOR_SIZE * 3 + I32_SIZE)..(data_ofs + VECTOR_SIZE * 3 + I32_SIZE * 2))?)?,
                    numfaces: i32_from_slice(&lump45_u8_vec.get((data_ofs + VECTOR_SIZE * 3 + I32_SIZE * 2)..(data_ofs + VECTOR_SIZE * 3 + I32_SIZE * 3))?)?,
                })
            }).collect())
        }

        pub fn get_lump_35(&self) -> Option<HashMap<i32, DGameLumpT>> {
            let lump45_u8_vec = self.get_lump(35)?;
            let lump_count = i32_from_slice(lump45_u8_vec.get(0..I32_SIZE)?)?;
            Some((0..lump_count).fold(HashMap::new(), |mut acc, i| {
                let dgame_lump_start_ofs = I32_SIZE + DGAME_LUMP_SIZE * i as usize;
                let dgame_lump_id = i32_from_slice(lump45_u8_vec.get(dgame_lump_start_ofs..(dgame_lump_start_ofs + I32_SIZE)).unwrap()).unwrap();
                let dgame_lump = DGameLumpT { 
                    id: dgame_lump_id, 
                    flags: u16_from_slice(lump45_u8_vec.get((dgame_lump_start_ofs + I32_SIZE)..(dgame_lump_start_ofs + I32_SIZE + U16_SIZE)).unwrap()).unwrap(), 
                    version: u16_from_slice(lump45_u8_vec.get((dgame_lump_start_ofs + I32_SIZE + U16_SIZE)..(dgame_lump_start_ofs + I32_SIZE + U16_SIZE * 2)).unwrap()).unwrap(), 
                    file_ofs: i32_from_slice(lump45_u8_vec.get((dgame_lump_start_ofs + I32_SIZE + U16_SIZE * 2)..(dgame_lump_start_ofs + I32_SIZE * 2 + U16_SIZE * 2)).unwrap()).unwrap(), 
                    file_len: i32_from_slice(lump45_u8_vec.get((dgame_lump_start_ofs + I32_SIZE * 2 + U16_SIZE * 2)..(dgame_lump_start_ofs + I32_SIZE * 3 + U16_SIZE * 2)).unwrap()).unwrap(),
                };

                acc.insert(dgame_lump_id, dgame_lump);
                acc
            }))
        } 

        pub fn get_prop_static(&self) -> Option<Vec<String>> {
            let prop_static_id = 1936749168;
            let lump35 = self.get_lump_35()?;
            let prop_static_info = lump35.get(&prop_static_id)?;
            let mut f = File::open(&self.path).ok()?;
            let ofs = prop_static_info.file_ofs;
            let len = prop_static_info.file_len;
            let prop_static_u8_vec = read_exact_from_file(&mut f, ofs as u64, len as usize)?;
            let dict_entries = i32_from_slice(prop_static_u8_vec.get(0..I32_SIZE)?)?;
            Some((0..dict_entries).filter_map(|i| {
                let prop_static_str_vec = read_exact_from_file(&mut f, (ofs as usize + I32_SIZE + i as usize * PS_NAME_SIZE) as u64, PS_NAME_SIZE)?;
                Some(String::from_utf8(prop_static_str_vec).ok()?.replace("\0", ""))
            }).collect())
        }

        pub fn get_lump_43(&self) -> Option<Vec<String>> {
            let lump43_u8_vec = self.get_lump(43)?;
            let lump43_str = String::from_utf8(lump43_u8_vec).ok()?;
            Some(lump43_str.split("\0").map(|slice_str| format!("{}.vmt", String::from(slice_str).to_lowercase())).collect())
        }

        pub fn download_content(&self, find_path: &Path, output_path: &Path) {
            let find_mats_path = find_path.join("materials");
            let find_sound_path = find_path.join("sound");
            let output_mats_path = output_path.join("materials");
            let output_sound_path = output_path.join("sound");
            let mut passed_path_strs: HashSet<&String> = HashSet::new();

            let lump0 = match self.get_lump_0() {
                Some(r) => r,
                None => return,
            };
            lump0.iter()
            .fold(Vec::<PathBuf>::new(), |mut acc, ent_info| {
                let model_val = match ent_info.get("model") {
                    Some(r) => r,
                    None => return acc,
                };

                let mut add_path_if_unique = |val| {
                    if !passed_path_strs.contains(val) {
                        passed_path_strs.insert(val);
                        acc.push(PathBuf::from(val));
                    }
                };
        
                if let Some(val) = ent_info.get("message") {
                    add_path_if_unique(val);
                }
                if let Some(val) = ent_info.get("noise1") {
                    add_path_if_unique(val);
                }
                if let Some(val) = ent_info.get("noise2") {
                    add_path_if_unique(val);
                }
        
                let fisrt_symbol = match model_val.get(0..1) {
                    Some(r) => r,
                    None => return acc,
                };
        
                if fisrt_symbol == "*" {
                    return acc;
                }
        
                if !passed_path_strs.contains(model_val) {
                    passed_path_strs.insert(model_val);
                    acc.push(PathBuf::from(model_val));
                }
        
                acc
            }).iter()
            .for_each(|file_path| {
                let path_ext_osstr = match file_path.extension() {
                    Some(r) => r,
                    None => {
                        return;
                    },
                };
                let path_ext_str = match path_ext_osstr.to_str() {
                    Some(r) => r,
                    None => {
                        return;
                    },
                };

                if path_ext_str == "vmt" {
                    match VMTInfo::new(file_path, &find_mats_path) {
                    Some(vmt_info) => {
                            vmt_info.download_with_def_keys(&find_mats_path, &output_mats_path);
                        },
                        None => {},
                    };
                } else if path_ext_str == "mdl" {
                    let mdl_abs_path = find_path.join(file_path);

                    match TexturesInfo::new(&mdl_abs_path) {
                        Some(tex_info) => {
                            tex_info.download(&find_path, &output_path);
                        },
                        None => {},
                    }
                } else if path_ext_str == "mp3" || path_ext_str == "wav" || path_ext_str == "ogg" {
                    let sound_input_file_path = find_sound_path.join(file_path);
                    let sound_output_file_path = output_sound_path.join(file_path);

                    match sound_output_file_path.parent() {
                        Some(sound_output_parent_dir_path) => {
                            let _ = fs::create_dir_all(sound_output_parent_dir_path);
                            let _ = fs::copy(sound_input_file_path, sound_output_file_path);
                        },
                        None => {},
                    };
                }
            });

            match self.get_prop_static() {
                Some(prop_static_vec) => {
                    prop_static_vec.iter().for_each(|mdl_path_str| {
                        let mdl_path = Path::new(mdl_path_str);
                        let mdl_abs_path = find_path.join(mdl_path);
                        match TexturesInfo::new(&mdl_abs_path) {
                            Some(tex_info) => {
                                tex_info.download(&find_path, &output_path);
                            },
                            None => {},
                        }
                    });
                },
                None => {},
            }

            let lump43 = match self.get_lump_43() {
                Some(r) => r,
                None => {
                    return;
                },
            };
            lump43.iter().for_each(|vmt_rel_path_str| {
                let vmt_rel_path = Path::new(vmt_rel_path_str);
                match VMTInfo::new(vmt_rel_path, &find_mats_path) {
                    Some(vmt_info) => {
                        vmt_info.download_with_def_keys(&find_mats_path, &output_mats_path);
                    },
                    None => {},
                };
            });
        }
    }
}
