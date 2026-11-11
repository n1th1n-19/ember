// Creates a dummy AppImage, finds it, uninstalls it (to trash). Own file only.
use ember_core::apps::{self, Source};
fn main() {
    let home = std::path::PathBuf::from(std::env::var("HOME").unwrap());
    let dir = home.join("Applications");
    let created_dir = !dir.exists();
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("ember-selftest.AppImage");
    std::fs::write(&f, b"\x7fELF\x02\x01\x01\0AI\x02\0dummy").unwrap();
    let app = apps::list().into_iter().find(|a| a.path.as_deref() == Some(f.as_path())).expect("dummy AppImage not listed");
    assert_eq!(app.source, Source::AppImage);
    let plan = apps::plan(&app);
    println!("found: {} | plan: button={} cmd={}", app.name, plan.button, plan.command);
    apps::uninstall(&app).expect("uninstall");
    assert!(!f.exists());
    println!("uninstalled: file moved to trash");
    let trash = home.join(".local/share/Trash");
    let _ = std::fs::remove_file(trash.join("files/ember-selftest.AppImage"));
    let _ = std::fs::remove_file(trash.join("info/ember-selftest.AppImage.trashinfo"));
    if created_dir { let _ = std::fs::remove_dir(&dir); }
    println!("cleaned up");
}
