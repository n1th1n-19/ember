// Read-only check of controls: `cargo run -p ember-core --example controls`
use ember_core::control::{cpu, power, services};

fn main() {
    zbus::block_on(async {
        println!("power: {:#?}", power::read().await);
        let user = services::list(services::Scope::User).await.map(|u| u.len());
        let sys = services::list(services::Scope::System).await;
        println!("user services: {user:?}");
        match sys {
            Ok(u) => println!("system services: {} e.g. {:?}", u.len(), u.iter().take(3).map(|u| (&u.name, &u.active, &u.file_state)).collect::<Vec<_>>()),
            Err(e) => println!("system services error: {e}"),
        }
    });
    println!("cpu: {:?}", cpu::read_state());
}
