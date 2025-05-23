pub mod reader {
    use std::{fs, fs::File, path::{Path, PathBuf}, mem::size_of, collections::{HashMap, HashSet}};
    use regex::Regex;
    use simple_utils::utils::{read_exact_from_file, read_segments_from_file, FromSlice};
    use source_mdl_mats_finder::finder::{TexturesInfo, VMTInfo};

    const HEADER_SIZE: usize = size_of::<DHeaderT>();
    const LUMP_SIZE: usize = size_of::<LumpT>();
    const DGAME_LUMP_SIZE: usize = size_of::<DGameLumpT>();
    const DMODEL_SIZE: usize = size_of::<DModelT>();
    const VECTOR_SIZE: usize = size_of::<Vector>();
    const I32_SIZE: usize = size_of::<i32>();
    const F32_SIZE: usize = size_of::<f32>();
    const U16_SIZE: usize = size_of::<u16>();
    const HEADER_LUMPS: usize = 64;
    const PS_NAME_SIZE: usize = 128;

    #[derive(Debug)]
    pub struct LumpT {
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
                x: f32::from_u8_slice(u8_vec.get(0..F32_SIZE)?)?,
                y: f32::from_u8_slice(u8_vec.get(F32_SIZE..(F32_SIZE * 2))?)?,
                z: f32::from_u8_slice(u8_vec.get((F32_SIZE * 2)..(F32_SIZE * 3))?)?,
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

    #[derive(Debug)]
    pub struct DHeaderT {
        path: String,
        ident: [u8; I32_SIZE],
        version: i32,
        lumps: [LumpT; HEADER_LUMPS],
        map_revision: i32,
    }

    fn get_bytes_4(bytes: &[u8], start: usize) -> Option<[u8; I32_SIZE]> {
        bytes[start..start + I32_SIZE].try_into().ok()
    }

    impl LumpT {
        fn new(header_bytes: &[u8], lump_num: usize) -> Option<Self> {
            let offset = I32_SIZE * 2 + lump_num * LUMP_SIZE;
            Some(Self {
                file_ofs: i32::from_u8_slice(header_bytes.get(offset..(offset + I32_SIZE))?)?,
                file_len: i32::from_u8_slice(header_bytes.get((offset + I32_SIZE)..(offset + I32_SIZE * 2))?)?,
                version: i32::from_u8_slice(header_bytes.get((offset + I32_SIZE * 2)..(offset + I32_SIZE * 3))?)?,
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
                version: i32::from_u8_slice(header_bytes.get(0..I32_SIZE)?)?,
                map_revision: i32::from_u8_slice(header_bytes.get(map_revision_ofs..(map_revision_ofs + I32_SIZE))?)?,
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

        pub fn get_lump_info(&self, lump_id: usize) -> Option<&LumpT> {
            Some(&self.lumps[lump_id])
        }

        pub fn get_lump_0(&self) -> Option<Vec<HashMap<String, String>>> {
            let lump_info = self.get_lump_info(0)?;
            let mut f = File::open(&self.path).ok()?;
            let lump0_vec = read_exact_from_file(&mut f, lump_info.file_ofs as u64, lump_info.file_len as usize)?;
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
            let mut f = File::open(&self.path).ok()?;
            let lump_info = self.get_lump_info(14)?;
            let size_vec = vec![VECTOR_SIZE, VECTOR_SIZE, VECTOR_SIZE, I32_SIZE, I32_SIZE, I32_SIZE];
            Some((0..(lump_info.file_len as usize / DMODEL_SIZE)).filter_map(|i| {
                let segments = read_segments_from_file(&mut f, (lump_info.file_ofs as usize + DMODEL_SIZE * i) as u64, &size_vec)?;
                Some(DModelT {
                    mins: Vector::from_u8_vec(&segments[0])?,
                    maxs: Vector::from_u8_vec(&segments[1])?,
                    origin: Vector::from_u8_vec(&segments[2])?,
                    headnode: i32::from_u8_slice(&segments[3])?,
                    firstface: i32::from_u8_slice(&segments[4])?,
                    numfaces: i32::from_u8_slice(&segments[5])?,
                })
            }).collect())
        }

        pub fn get_lump_35(&self) -> Option<HashMap<i32, DGameLumpT>> {
            let mut f = File::open(&self.path).ok()?;
            let lump_info = self.get_lump_info(35)?;
            let lump_ofs = lump_info.file_ofs;
            let lump_count = i32::from_u8_slice(&read_exact_from_file(&mut f, lump_ofs as u64, I32_SIZE)?)?;
            let size_vec = vec![I32_SIZE, U16_SIZE, U16_SIZE, I32_SIZE, I32_SIZE];
            Some((0..lump_count).filter_map(|i| {
                let segments = read_segments_from_file(&mut f, (lump_ofs as usize + I32_SIZE + DGAME_LUMP_SIZE * i as usize) as u64, &size_vec)?;
                let id = i32::from_u8_slice(&segments[0])?;
                Some((
                    id,
                    DGameLumpT {
                        id,
                        flags: u16::from_u8_slice(&segments[1])?,
                        version: u16::from_u8_slice(&segments[2])?,
                        file_ofs: i32::from_u8_slice(&segments[3])?,
                        file_len: i32::from_u8_slice(&segments[4])?,
                    },
                ))
            }).collect())
        }

        pub fn get_prop_static(&self) -> Option<Vec<String>> {
            let mut f = File::open(&self.path).ok()?;
            let prop_static_id = 1936749168;
            let lump35 = self.get_lump_35()?;
            let prop_static_info = lump35.get(&prop_static_id)?;
            let ofs = prop_static_info.file_ofs;
            let dict_entries = i32::from_u8_slice(&read_exact_from_file(&mut f, ofs as u64, I32_SIZE)?)?;
            Some((0..dict_entries).filter_map(|i| {
                Some(String::from_utf8(read_exact_from_file(&mut f, (ofs as usize + I32_SIZE + i as usize * PS_NAME_SIZE) as u64, PS_NAME_SIZE)?).ok()?.replace("\0", ""))
            }).collect())
        }

        pub fn get_lump_43(&self) -> Option<Vec<String>> {
            let mut f = File::open(&self.path).ok()?;
            let lump_info = self.get_lump_info(43)?;
            let lump43_str = String::from_utf8(read_exact_from_file(&mut f, lump_info.file_ofs as u64, lump_info.file_len as usize)?).ok()?;
            Some(lump43_str
                .split_terminator('\0')
                .map(|s| format!("{}.vmt", s.to_lowercase()))
                .collect())
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
                let ext = match file_path.extension().and_then(|ext| ext.to_str()) {
                    Some(ext) => ext,
                    None => return,
                };
            
                match ext {
                    "vmt" => {
                        if let Some(vmt_info) = VMTInfo::new(file_path, &find_mats_path) {
                            vmt_info.download_with_def_keys(&find_mats_path, &output_mats_path);
                        }
                    }
                    "mdl" => {
                        if let Some(tex_info) = TexturesInfo::new(&find_path.join(file_path)) {
                            tex_info.download(&find_path, &output_path);
                        }
                    }
                    ext if ["mp3", "wav", "ogg"].contains(&ext) => {
                        let sound_output_file_path = output_sound_path.join(file_path);
                        if let Some(parent_dir) = sound_output_file_path.parent() {
                            let _ = fs::create_dir_all(parent_dir);
                            let _ = fs::copy(find_sound_path.join(file_path), sound_output_file_path);
                        }
                    }
                    _ => {}
                }
            });

            if let Some(prop_static_vec) = self.get_prop_static() {
                prop_static_vec.iter().for_each(|mdl_path_str| {
                    let mdl_path = Path::new(mdl_path_str);
                    let mdl_abs_path = find_path.join(mdl_path);

                    if let Some(tex_info) = TexturesInfo::new(&mdl_abs_path) {
                        tex_info.download(&find_path, &output_path);
                    }
                });
            }

            let lump43 = match self.get_lump_43() {
                Some(r) => r,
                None => {
                    return;
                },
            };
            lump43.iter().for_each(|vmt_rel_path_str| {
                if let Some(vmt_info) = VMTInfo::new(Path::new(vmt_rel_path_str), &find_mats_path) {
                    vmt_info.download_with_def_keys(&find_mats_path, &output_mats_path);
                }
            });
        }
    }
}
