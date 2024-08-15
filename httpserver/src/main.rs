// main.rs
mod config;
mod handler;
mod router;
mod server;

use config::read_config;
use server::Server;

fn main() {
    // Load configuration
    let configs = read_config();
    match configs {
        Some(config) => {
            // Start servers on configured ports
            let server = Server::new(config);
            // Run the server
            server.run();
        }
        _ => eprintln!("⚠️ Incorrect configuration⚠️"),
    }
}
