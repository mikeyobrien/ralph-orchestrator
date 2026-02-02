use ralph_core::EventParser;
use std::hint::black_box;
use std::time::Instant;

fn run_parse_baseline(iterations: u64, payload: &str) {
    let start = Instant::now();
    for _ in 0..iterations {
        let evidence = EventParser::parse_backpressure_evidence(black_box(payload))
            .expect("backpressure evidence should parse");
        black_box(evidence);
    }
    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_secs_f64() * 1_000_000_000.0 / iterations as f64;

    println!("\n=== event_parser_backpressure_baseline ===");
    println!("iterations: {}", iterations);
    println!("total: {:?}", elapsed);
    println!("ns/op: {:.2}", ns_per_op);
    println!("=========================================\n");
}

fn run_backpressure_baseline(iterations: u64, payload: &str) {
    let evidence = EventParser::parse_backpressure_evidence(payload)
        .expect("backpressure evidence should parse");

    let start = Instant::now();
    for _ in 0..iterations {
        black_box(evidence.all_passed());
    }
    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_secs_f64() * 1_000_000_000.0 / iterations as f64;

    println!("\n=== backpressure_all_passed_baseline ===");
    println!("iterations: {}", iterations);
    println!("total: {:?}", elapsed);
    println!("ns/op: {:.2}", ns_per_op);
    println!("=======================================\n");
}

fn main() {
    let payload = "tests: pass\nlint: pass\ntypecheck: pass\naudit: pass\ncoverage: pass\ncomplexity: 7\nduplication: pass\nperformance: pass";

    run_parse_baseline(200_000, payload);
    run_backpressure_baseline(500_000, payload);
}
