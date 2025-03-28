use std::env;
use std::path::Path;
use source_mdl_mats_finder::finder::TexturesInfo;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: {} <find_path> <output_path> <mdl_file_path>", args[0]);
        std::process::exit(1);
    }

    let find_path = Path::new(&args[1]);
    let output_path = Path::new(&args[2]);
    let mdl_file_path = Path::new(&args[3]);

    let texture_info = match TexturesInfo::new(mdl_file_path) {
        Some(r) => r,
        None => {
            eprintln!("Не удалось создать TexturesInfo!");
            std::process::exit(1);
        }
    };
    texture_info.download(find_path, output_path);
}