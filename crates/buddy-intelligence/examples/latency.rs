//! Warm local composition timing only; this is not microphone-to-speaker latency.
fn main() {
    let inputs = [
        "What is photosynthesis?",
        "Explain voltage",
        "What is the capital of France?",
        "and India",
        "What is 12 plus 3 times 4?",
        "how do you spell electricity",
        "Can you help me please?",
    ];
    let mut engine = buddy_intelligence::Intelligence::default();
    let mut values = Vec::new();
    for n in 0..5000 {
        let input = inputs[n % inputs.len()];
        let start = std::time::Instant::now();
        let analysis = buddy_intelligence::analyze(input);
        let result = engine.answer(input);
        std::hint::black_box((analysis, result));
        if n >= 100 {
            values.push(start.elapsed().as_nanos());
        }
    }
    values.sort_unstable();
    println!("Warm local analysis/composition, 4900 samples: p50 {:.3} ms; p95 {:.3} ms; p99 {:.3} ms. No audio, network or model inference included.",values[values.len()/2] as f64/1e6,values[values.len()*95/100] as f64/1e6,values[values.len()*99/100] as f64/1e6);
}
