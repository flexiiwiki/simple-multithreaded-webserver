use simple_multithreaded_webserver::ServerConfig;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    let ip = &args[0];
    let port: u16 = args[1].parse().unwrap();
    let thread_nums: usize = args[2].parse().unwrap();

    let config = ServerConfig::new(ip, port, thread_nums, true, None, None).unwrap();

    config.start();
}
