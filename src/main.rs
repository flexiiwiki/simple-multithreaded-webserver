use simple_multithreaded_webserver::ServerConfig;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    println!("{:#?}", args);

    let ip = &args[1];
    let port: u16 = args[2].parse::<u16>().unwrap();
    let thread_nums: usize = args[3].parse::<usize>().unwrap();

    let config = ServerConfig::new(ip, port, thread_nums, true, None, None).unwrap();

    config.start();
}
