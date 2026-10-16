//! Read-only public source adapter check; prints counts, never transcript contents.
#[tokio::main]
async fn main() {
    let mut failed = 0;
    for source in buddy_core::local_learning::bundled_sources() {
        match buddy_core::local_learning::fetch(&source).await {
            Ok(documents) => println!(
                "PASS {}: {} documents, {} bytes",
                source.name,
                documents.len(),
                documents.iter().map(|d| d.text.len()).sum::<usize>()
            ),
            Err(error) => {
                failed += 1;
                println!("FAIL {}: {}", source.name, error);
            }
        }
    }
    if failed > 0 {
        std::process::exit(1);
    }
}
