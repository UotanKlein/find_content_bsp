use std::env;
use std::path::Path;
use source_bsp_reader::reader::DHeaderT;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: {} <find_path> <output_path> <bsp_file_path>", args[0]);
        std::process::exit(1);
    }

    let find_path = Path::new(&args[1]);
    let output_path = Path::new(&args[2]);
    let bsp_file_path = Path::new(&args[3]);

    let dheader_t = match DHeaderT::new(bsp_file_path) {
        Some(r) => r,
        None => {
            return;
        },
    };

    dheader_t.download_content(find_path, output_path);
}