//! `wtg-rs`: checks trigger files (`war3map.wtg`) and converts them to
//! text and back. See `usage`.

use std::process::ExitCode;

use wtg_rs::{TriggerData, text};

fn usage() {
    eprintln!("usage: wtg-rs <TriggerData.txt> <war3map.wtg>                    check: read and write back");
    eprintln!("       wtg-rs --text <TriggerData.txt> <war3map.wtg>             the text form, to stdout");
    eprintln!("       wtg-rs --from-text <TriggerData.txt> <in.ini> <out.wtg>   the text form back to a file");
    eprintln!();
    eprintln!("  TriggerData.txt is World Editor's UI\\TriggerData.txt (the game's, or a modded");
    eprintln!("  editor's): how many parameters each function takes isn't in the file.");
}

fn data(path: &str) -> Result<TriggerData, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    Ok(TriggerData::parse(&String::from_utf8_lossy(&bytes)))
}

fn run(args: &[&str]) -> Result<(), String> {
    match args {
        ["--text", d, path] => {
            let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
            let f = wtg_rs::parse_any(&bytes, &data(d)?).map_err(|e| format!("{path}: {e}"))?;
            print!("{}", text::to_text_any(&f).map_err(|e| format!("{path}: {e}"))?);
            Ok(())
        }
        ["--from-text", d, input, output] => {
            let src = std::fs::read_to_string(input).map_err(|e| format!("{input}: {e}"))?;
            let p = text::from_text(&src, Some(&data(d)?));
            let Some(f) = p.to_file() else {
                let lines: Vec<String> = p.errors.iter().map(|e| format!("{input}:{}:{}: {}", e.line, e.col, e.message)).collect();
                return Err(lines.join("\n"));
            };
            std::fs::write(output, f.to_bytes().map_err(|e| e.to_string())?).map_err(|e| format!("{output}: {e}"))
        }
        [d, path] if !d.starts_with('-') => {
            let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
            let f = wtg_rs::parse_any(&bytes, &data(d)?).map_err(|e| format!("{path}: {e}"))?;
            if f.to_bytes().ok() != Some(bytes) {
                return Err(format!("{path}: differs after writing"));
            }
            match f {
                wtg_rs::File::Classic(t) => println!(
                    "{path}: version {}, {} categories, {} variables, {} triggers",
                    t.version,
                    t.categories.len(),
                    t.variables.len(),
                    t.triggers.len()
                ),
                wtg_rs::File::Tree(t) => println!("{path}: 1.31 layout, version {}, {} elements", t.version, t.elements.len()),
            }
            Ok(())
        }
        _ => {
            usage();
            Err(String::new())
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("{e}");
            }
            ExitCode::FAILURE
        }
    }
}
