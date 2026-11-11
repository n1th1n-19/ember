// Read-only: everything the Quick panel shows.
use ember_core::control::{bluetooth, desktop, network, sound};
fn main() {
    for k in [sound::Kind::Output, sound::Kind::Input] {
        println!("{k:?}: {:?}", sound::list(k).map(|v| v.into_iter().map(|d| (d.description, (d.volume * 100.0).round(), d.muted, d.default)).collect::<Vec<_>>()));
    }
    let n = network::read().unwrap();
    println!("links {:?}\nwifi {} airplane {} nets {:?}", n.links, n.wifi_enabled, n.airplane, n.networks.iter().take(4).map(|w| (&w.ssid, w.signal, w.known)).collect::<Vec<_>>());
    zbus::block_on(async {
        let b = bluetooth::read().await.unwrap();
        println!("bt powered {} {:?}", b.powered, b.devices.iter().map(|d| (&d.name, d.connected, d.battery, &d.icon)).collect::<Vec<_>>());
        println!("brightness {:?}", desktop::read_brightness().await);
    });
    let c = ember_core::calendar::now();
    println!("now {c:?}");
}
