use std::process::ExitCode;

use inklift_cli::{parse_args, run};

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let config = match parse_args(&argv) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    match run(&config) {
        Ok(report) => {
            if !config.quiet {
                match report.ink_color {
                    Some([r, g, b]) => println!(
                        "[{}] ink {:.1}% of page, pen rgb({:.0}, {:.0}, {:.0})",
                        report.source,
                        report.coverage * 100.0,
                        r * 255.0,
                        g * 255.0,
                        b * 255.0
                    ),
                    None => println!(
                        "[{}] ink {:.1}% of page",
                        report.source,
                        report.coverage * 100.0
                    ),
                }
                for p in &report.written {
                    println!("wrote {}", p.display());
                }
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("inklift: {e}");
            ExitCode::FAILURE
        }
    }
}
