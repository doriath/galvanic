use crate::commands::Commands;
use ayysee_compiler::generate_program;
use ayysee_parser::grammar::ProgramParser;
use clap::Parser;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

mod commands;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = commands::Args::parse();
    match args.command {
        Commands::Compile { file, output } => {
            let file_contents = tokio::fs::read_to_string(file).await.unwrap();

            let parser = ProgramParser::new();

            let parsed = parser.parse(&file_contents).unwrap();

            match output {
                commands::CompilationType::Ast => println!("{:#?}", parsed),
                commands::CompilationType::Mips => {
                    let compiled = generate_program(parsed)?;
                    println!("{}", compiled);
                }
            }
        }
        Commands::Format { files } => {
            if files.is_empty() {
                let mut content: String = "".to_string();
                tokio::io::stdin().read_to_string(&mut content).await?;
                let parser = ProgramParser::new();
                // TODO: need to figure out lifetimes to properly handle error
                let parsed = parser.parse(&content).unwrap();
                let formatted = ayysee_parser::format::format(parsed)?;
                tokio::io::stdout()
                    .write_all(&formatted.into_bytes())
                    .await?;
            } else {
                for file in files {
                    let file_contents = tokio::fs::read_to_string(&file).await?;
                    let parser = ProgramParser::new();
                    // TODO: need to figure out lifetimes to properly handle error
                    let parsed = parser.parse(&file_contents).unwrap();
                    let formatted = ayysee_parser::format::format(parsed)?;
                    tokio::fs::write(&file, formatted).await?;
                }
            }
        }
    }

    Ok(())
}
