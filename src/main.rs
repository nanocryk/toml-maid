use toms_maid::find_files_recursively;

use {
    colored::*,
    structopt::StructOpt,
    toms_maid::{Config, Opt, ProcessedConfig, Res},
};

fn main() -> Res<()> {
    let mut opt = Opt::from_args();

    let config = Config::read_from_file().unwrap_or_else(|| {
        if !opt.silent {
            println!(
                "{}",
                "No 'toms-maid.toml' in this directory and its parents, using default config.\n"
                    .yellow()
            );
        }

        Config::default()
    });

    let config: ProcessedConfig = config.into();

    if opt.files.is_empty() && opt.folder.is_empty() {
        opt.folder.push(std::env::current_dir()?);
    }

    for folder in opt.folder {
        let files = find_files_recursively(folder, "toml", !opt.silent);
        opt.files.extend(files);
    }

    for file in opt.files {
        config.process_file(file, opt.check, !opt.silent)?;
    }

    Ok(())
}
