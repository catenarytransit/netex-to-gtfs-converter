mod gtfs;
mod parser;

use clap::Parser;
use std::path::PathBuf;
use std::error::Error;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The input NeTEx XML file
    #[arg(short, long)]
    input: PathBuf,

    /// The output GTFS directory
    #[arg(short, long, default_value = "gtfs_out")]
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    
    println!("Parsing NeTEx file: {:?}", args.input);
    
    let model = parser::parse_netex(&args.input)?;
    
    println!("Exporting to GTFS: {:?}", args.output);
    parser::export_gtfs(&model, &args.output)?;
    
    println!("Done!");
    Ok(())
}
