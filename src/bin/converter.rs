use anyhow::Result;
use clap::App;
use itertools::__std_iter::FromIterator;
use std::env;

fn main() {
    let res = work();
    match res {
        Ok(_) => eprintln!("\nconversion successfully finished :)"),
        Err(error) => eprintln!("\nAn error occured: {}", error),
    }
}

// used to have the convenience of ? for error handling
fn work() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let arg_blocks: Vec<Vec<&String>> =
        args.split(|s| s == "!").map(|slice| Vec::from_iter(slice)).collect();

    let _main_app_arguments = App::new("Raw Image / Video Converter")
        .version("0.1")
        .about("convert raw footage from AXIOM cameras into other formats.")
        .after_help("try to build a chain by separating subcommands with !")
        .get_matches_from(&arg_blocks[0]);

    for block in arg_blocks {
        println!("{:?}", block)
    }

    Ok(())
}
