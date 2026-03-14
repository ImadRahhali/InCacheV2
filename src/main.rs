mod protocol;
mod store;
mod commands;
mod server;

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    let mut port: u16 = 6399;
    let mut host = "0.0.0.0".to_string();
    let mut password: Option<String> = None;

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => { port = args[i + 1].parse().unwrap_or(6399); i += 2; }
            "--host" => { host = args[i + 1].clone(); i += 2; }
            "--requirepass" => { password = Some(args[i + 1].clone()); i += 2; }
            _ => { i += 1; }
        }
    }

    server::run_server(&host, port, password);
}
