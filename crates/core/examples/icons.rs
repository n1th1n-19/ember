// Read-only: icon coverage for apps and running processes.
fn main() {
    let apps = ember_core::apps::list();
    println!("apps with icon: {}/{}", apps.iter().filter(|a| a.icon.is_some()).count(), apps.len());
    let map = ember_core::apps::launcher_icons();
    let mut p = ember_core::control::procs::Procs::new();
    std::thread::sleep(std::time::Duration::from_millis(300));
    let rows = p.list();
    let hit: Vec<_> = rows.iter().filter(|r| map.contains_key(&r.name.to_lowercase())).map(|r| r.name.clone()).collect();
    println!("procs with icon: {}/{}  e.g. {:?}", hit.len(), rows.len(), &hit[..hit.len().min(14)]);
}
