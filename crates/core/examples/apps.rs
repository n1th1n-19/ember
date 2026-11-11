// Read-only: `cargo run -p ember-core --example apps`
fn main() {
    let t = std::time::Instant::now();
    let apps = ember_core::apps::list();
    println!("{} apps in {:?}", apps.len(), t.elapsed());
    for s in ember_core::apps::Source::ALL {
        let of: Vec<_> = apps.iter().filter(|a| a.source == s).collect();
        println!("{:9} {:3}  e.g. {:?}", s.name(), of.len(), of.iter().take(4).map(|a| (&a.name, &a.id)).collect::<Vec<_>>());
    }
    for name in ["Blender", "OBS Studio", "Firefox", "Superset", "COSMIC Files", "Terminal", "Blip"] {
        if let Some(a) = apps.iter().find(|a| a.name.contains(name)) {
            let p = ember_core::apps::plan(a);
            println!("{name}: {:?} button={} cmd={} blocked={:?}", a.source, p.button, p.command, p.blocked);
        }
    }
}
