// Prints one real snapshot: `cargo run -p ember-core --example dump`
fn main() {
    let mut s = ember_core::Sampler::new();
    std::thread::sleep(std::time::Duration::from_millis(800));
    println!("{:#?}", s.sample());
}
