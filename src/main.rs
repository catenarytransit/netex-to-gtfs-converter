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

    /// Default agency name to use when no operator name is available
    #[arg(long, default_value = "Default Agency")]
    default_agency_name: String,

    /// Default time zone (IANA) to use for agencies
    #[arg(long, default_value = "Europe/Rome")]
    default_agency_timezone: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    
    println!("Parsing NeTEx file: {:?}", args.input);

    let model = parser::parse_netex(
        &args.input,
        &args.default_agency_name,
        &args.default_agency_timezone,
    )?;
    
    println!("Exporting to GTFS: {:?}", args.output);
    parser::export_gtfs(&model, &args.output)?;
    
    println!("Done!");
    Ok(())
}
