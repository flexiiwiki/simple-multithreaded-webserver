mod defaults;

use chrono::{Duration, Utc};
use flate2::write::GzEncoder;
use flate2::Compression;
use httparse::Request;
use httpdate::fmt_http_date;
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::{AddrParseError, Ipv4Addr, TcpListener, TcpStream},
    path::Path,
    sync::{mpsc, Arc, Mutex},
    thread,
};

struct ThreadPool {
    workers: Vec<ThreadWorker>,
    sender: Option<mpsc::Sender<Job>>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
    pub fn new(size: usize) -> Self {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers: Vec<ThreadWorker> = Vec::with_capacity(size);

        for id in 0..size {
            let worker_result = ThreadWorker::new(id, Arc::clone(&receiver));
            if let Ok(worker) = worker_result {
                workers.push(worker);
            } else {
                eprintln!("Unable to spawn thread! thread\nid: {id}\n");
                continue;
            }
        }

        ThreadPool {
            workers,
            sender: Some(sender),
        }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());
        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);

            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}
struct ThreadWorker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}
impl ThreadWorker {
    fn new(id: usize, reciever: Arc<Mutex<mpsc::Receiver<Job>>>) -> Result<Self, std::io::Error> {
        let thread = thread::Builder::new().spawn(move || loop {
            let message = reciever
                .lock()
                .expect("Receiving failed, other thread panic.")
                .recv();

            match message {
                Ok(job) => {
                    println!("Worker {id} got a job; executing.");
                    job();
                }
                Err(_) => {
                    println!("Worker {id} disconnected; shutting down.");
                    break;
                }
            }
        })?;

        Ok(ThreadWorker {
            id,
            thread: Some(thread),
        })
    }
}

#[derive(Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub path: String,
    pub status_text: String,
    pub compress: bool,
}

impl HttpResponse {
    #[allow(dead_code)]
    pub fn new(path: String, status: u16, status_text: String, compress: bool) -> Self {
        HttpResponse {
            path,
            status,
            status_text,
            compress,
        }
    }

    pub fn response(&mut self) -> Vec<u8> {
        let one_week_from_now = Utc::now() + Duration::days(7);

        let one_week_from_now_formatted = fmt_http_date(one_week_from_now.into());
        //If the path has no file name in it, append index.html.
        if !self.path.contains('.') {
            self.path = format!["{}/index.html", &self.path];
        }
        //reads file contents, if file fails to read print error and give empty contents.
        let mut response_body_content_file: Vec<u8> =
            fs::read(&self.path).unwrap_or_else(|error| {
                eprintln!("Failed to read file, error: {error} path:{}", &self.path);
                Vec::new()
            });

        //creates empty string literal for the compress request line, if it requester doesnt support gzip, or if the encoder throws an error, it skips the compression.
        let mut compress_request_line = "";
        if self.compress
            && Self::encoder(&mut response_body_content_file, &mut compress_request_line).is_err()
        {
            eprintln!("Error in compression encoder! Skipping compression.");
        }

        //builds output string, minus the contents. turns it into a byte array and then into a vector
        let mut output = format![
            "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nExpires:{}{}\r\n\r\n",
            self.status,
            self.status_text,
            response_body_content_file.len(),
            one_week_from_now_formatted,
            compress_request_line
        ]
        .as_bytes()
        .to_vec();

        //appends response body to the output, then returns the final vector
        output.extend(response_body_content_file);
        output
    }

    fn encoder(
        response_body_content_file: &mut Vec<u8>,
        compress_request_line: &mut &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut gzip_encoder = GzEncoder::new(Vec::new(), Compression::default());
        gzip_encoder.write_all(response_body_content_file)?;
        *response_body_content_file = gzip_encoder.finish()?;
        *compress_request_line = "\r\nContent-Encoding: gzip";
        Ok(())
    }
}

pub struct ServerConfig {
    ip_address: Ipv4Addr,
    port: u16,
    thread_amount: usize,
    enable_gzip: bool,
    not_found_response: HttpResponse,
    forbidden_response: HttpResponse,
}

impl ServerConfig {
    pub fn new(
        input_ip: &str,
        port: u16,
        thread_amount: usize,
        enable_gzip: bool,
        not_found: Option<HttpResponse>,
        forbidden: Option<HttpResponse>,
    ) -> Result<Self, AddrParseError> {
        if let Some(not_found_response) = not_found {
            if let Some(forbidden_response) = forbidden {
                return Ok(Self {
                    ip_address: input_ip.parse::<Ipv4Addr>()?,
                    port,
                    thread_amount,
                    enable_gzip,
                    not_found_response,
                    forbidden_response,
                });
            }
        }
        Ok(Self {
            ip_address: input_ip.parse::<Ipv4Addr>()?,
            port,
            thread_amount,
            enable_gzip,
            ..Default::default()
        })
    }
    pub fn start(self) {
        let listener =
            TcpListener::bind((self.ip_address, self.port)).expect("Failed to bind to socket.");

        let pool = ThreadPool::new(self.thread_amount);

        let arc = Arc::new(self);

        for stream_result in listener.incoming() {
            let arc = Arc::clone(&arc);
            match stream_result {
                Err(error) => eprintln!("Connection attempt failed. Error: {error}"),
                Ok(stream) => pool.execute(move || {
                    if let Err(error) = arc.handle_connection(stream) {
                        eprintln!("Error in connection handling, going to next stream: {error}");
                    }
                }),
            };
        }

        println!("Shutting down");
    }

    fn handle_connection(&self, mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
        let mut buf_reader = BufReader::new(&mut stream);
        let buffer: &[u8] = buf_reader.fill_buf()?;
        let http_response = self.response_from_request(buffer)?.response();
        let len = buffer.len();

        buf_reader.get_mut().write_all(&http_response)?;

        buf_reader.consume(len);
        Ok(())
    }

    fn response_from_request(
        &self,
        buffer: &[u8],
    ) -> Result<HttpResponse, Box<dyn std::error::Error>> {
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut request = httparse::Request::new(&mut headers);
        request.parse(buffer)?;
        let mut response = HttpResponse::default();
        let mut path_output: String = String::from("index.html");
        if let Some(path) = request.path {
            request.path = Some(if path == "/" {
                &path_output
            } else if !path.contains('.') {
                path_output = format!["{}/{path_output}", &path[1..]];
                &path_output
            } else {
                &path[1..]
            });
        }
        match request {
            Request {
                path: Some(path),
                method: Some(method),
                headers,
                ..
            } if Path::new(&path).exists() && method == "GET" => {
                response.status = 200;
                response.status_text = String::from("OK");
                response.path = path.to_string();

                if headers.iter().any(|&header| {
                    header.name == "Accept-Encoding"
                        && std::str::from_utf8(header.value)
                            .unwrap_or("")
                            .contains("gzip")
                }) {
                    response.compress = self.enable_gzip;
                }
            }
            Request {
                method: Some(method),
                ..
            } if method != "GET" => response = self.forbidden_response.clone(),
            _ => response = self.not_found_response.clone(),
        }

        Ok(response)
    }
}
