use {
    colored::*,
    structopt::StructOpt,
    toml_maid::{Config, Opt, Res},
};

fn main() -> Res {
    let opt = Opt::from_args();

    let config = Config::read_from_file().unwrap_or_else(|| {
        if !opt.silent {
            println!(
                "{}",
                "No 'toml-maid.toml' in this directory and its parents, using default config.\n"
                    .yellow()
            );
        }

        Config::default()
    });

    toml_maid::run(opt, config)
}
