use std::process::ExitCode;

use inklift_cli::{parse_args, run};

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();

    if argv.first().map(String::as_str) == Some("shot") {
        return run_shot_command(&argv[1..]);
    }

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

#[cfg(feature = "shot")]
fn run_shot_command(argv: &[String]) -> ExitCode {
    use inklift_cli::{ShotOutcome, parse_shot_args, run_shot};

    let config = match parse_shot_args(argv) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    match run_shot(&config) {
        Ok(ShotOutcome::Cancelled) => {
            if !config.quiet {
                eprintln!("cancelled");
            }
            ExitCode::FAILURE
        }
        Ok(ShotOutcome::Captured(report)) => {
            // The region is the machine-readable result, so it alone goes to
            // stdout and can be piped straight back into --region.
            println!("{}", report.region);
            if !config.quiet {
                let [r, g, b] = report.ink_color;
                eprintln!(
                    "captured {} - ink {:.1}%, pen rgb({:.0}, {:.0}, {:.0})",
                    report.region,
                    report.coverage * 100.0,
                    r * 255.0,
                    g * 255.0,
                    b * 255.0
                );
                for p in &report.written {
                    eprintln!("wrote {}", p.display());
                }
                if let Some(raw) = &report.raw {
                    eprintln!("wrote {} (raw capture)", raw.display());
                }
                match &report.clipboard {
                    Some(Ok(())) => eprintln!("copied to clipboard"),
                    Some(Err(e)) => eprintln!("clipboard unavailable: {e}"),
                    None => {}
                }
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("inklift shot: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(feature = "shot"))]
fn run_shot_command(_argv: &[String]) -> ExitCode {
    eprintln!(
        "`shot` needs the screen-capture path, which this build does not include.\n\
         Rebuild with: cargo build --release --features inklift-cli/shot"
    );
    ExitCode::FAILURE
}
