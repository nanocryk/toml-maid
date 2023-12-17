use {
    colored::*,
    structopt::StructOpt,
    toms_maid::{Config, Opt, Res},
};

fn main() -> Res {
    let opt = Opt::from_args();

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

    toms_maid::run(opt, config)
}
