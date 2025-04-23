use std::fs::File;
use std::io::Write;

use clap::Parser;

#[derive(Parser)]
#[command(name = "mm3u-rs")]
#[command(author, version, about)]
struct Cli {
    #[arg(long, short)]
    list: String,
    #[arg(long, short)]
    dir: String,

    #[arg(short = 'p', long = "parallel", help = "Enable parallel mode")]
    is_parallel: bool,

    #[arg(
        short = 'a',
        long = "absolute",
        help = "Output paths as absolute paths",
        default_value_t = false
    )]
    is_absolute_output: bool,

    #[arg(short = 'o', long = "output", required = false)]
    output_file: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    let user_list = mm3u_rs::get_user_list(&cli.list);
    let local_song = mm3u_rs::get_local_song(&cli.dir);

    let (hit_match, _) = {
        if cli.is_parallel {
            mm3u_rs::list_to_m3u_fuzzy_parallel(&user_list, &local_song, cli.is_absolute_output)
        } else {
            mm3u_rs::list_to_m3u_fuzzy(&user_list, &local_song, cli.is_absolute_output)
        }
    };
    if let Some(path) = cli.output_file {
        let mut file = File::create(&path).unwrap();
        writeln!(file, "#EXTM3U").unwrap();
        for line in hit_match {
            writeln!(file, "{}", line).unwrap();
        }
    }
}
