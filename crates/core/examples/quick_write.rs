// Writes current values back unchanged: proves the write paths without changing anything.
use ember_core::control::{desktop, sound};
fn main() {
    let out = sound::list(sound::Kind::Output).unwrap();
    let d = out.iter().find(|d| d.default).unwrap();
    println!("set volume {} -> {:?}", d.volume, sound::set_volume(sound::Kind::Output, &d.name, d.volume));
    println!("after: {:?}", sound::list(sound::Kind::Output).unwrap().iter().find(|d| d.default).map(|d| d.volume));
    zbus::block_on(async {
        let b = desktop::read_brightness().await;
        if let Some(v) = b.display {
            println!("set display {v} -> {:?}", desktop::set_display(v).await);
        }
        if let Some(v) = b.keyboard {
            println!("set keyboard {v} -> {:?}", desktop::set_keyboard(v).await);
        }
        println!("after: {:?}", desktop::read_brightness().await);
    });
}
