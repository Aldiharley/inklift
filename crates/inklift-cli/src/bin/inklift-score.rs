use std::process::ExitCode;

use inklift_cli::{parse_score_args, run_score};

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let config = match parse_score_args(&argv) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    let summary = match run_score(&config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("inklift-score: {e}");
            return ExitCode::FAILURE;
        }
    };

    if summary.rows.is_empty() {
        eprintln!("inklift-score: no result/ground-truth pairs found");
        for name in &summary.unmatched {
            eprintln!("  no ground truth for {name}");
        }
        return ExitCode::FAILURE;
    }

    println!("{:<28} {:>8} {:>8} {:>8} {:>8}", "image", "FM", "p-FM", "PSNR", "DRD");
    println!("{}", "-".repeat(64));
    for row in &summary.rows {
        println!(
            "{:<28} {:>8.2} {:>8.2} {:>8.2} {:>8.4}",
            row.name, row.scores.fm, row.scores.p_fm, row.scores.psnr, row.scores.drd
        );
    }
    println!("{}", "-".repeat(64));
    println!(
        "{:<28} {:>8.2} {:>8.2} {:>8.2} {:>8.4}",
        format!("mean of {}", summary.rows.len()),
        summary.mean_fm,
        summary.mean_p_fm,
        summary.mean_psnr,
        summary.mean_drd
    );
    if summary.exact_matches > 0 {
        println!("({} exact match(es) excluded from the PSNR mean)", summary.exact_matches);
    }
    for name in &summary.unmatched {
        eprintln!("warning: no ground truth for {name}");
    }
    ExitCode::SUCCESS
}
