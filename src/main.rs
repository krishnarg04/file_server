use std::{
    collections::HashMap,
    env,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::Arc,
};


mod thread_pool {
    use std::{
        sync::{mpsc, Arc, Mutex},
        thread,
    };

    pub struct ThreadPool {
        workers: Vec<Worker>,
        sender: mpsc::Sender<Job>,
    }

    type Job = Box<dyn FnOnce() + Send + 'static>;

    impl ThreadPool {
        pub fn new(size: usize) -> ThreadPool {
            assert!(size > 0);

            let (sender, receiver) = mpsc::channel();
            let receiver = Arc::new(Mutex::new(receiver));
            let mut workers = Vec::with_capacity(size);

            for id in 0..size {
                workers.push(Worker::new(id, Arc::clone(&receiver)));
            }

            ThreadPool { workers, sender }
        }

        pub fn execute<F>(&self, f: F)
        where
            F: FnOnce() + Send + 'static,
        {
            let job = Box::new(f);
            self.sender.send(job).unwrap();
        }
    }

    struct Worker {
        id: usize,
        thread: thread::JoinHandle<()>,
    }

    impl Worker {
        fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
            let thread = thread::spawn(move || loop {
                let job = receiver.lock().unwrap().recv().unwrap();
                println!("Worker {} got a job; executing.", id);
                job();
            });

            Worker { id, thread }
        }
    }
}

struct File {
    name: String,
    isdir: bool,
    size: u64,
}

enum FileType {
    File,
    Directory,
    NotFound,
}

fn handle_client(mut stream: TcpStream, current_dir: Arc<PathBuf>) {
    let mut buffer = [0; 1024];
    if let Err(e) = stream.read(&mut buffer) {
        eprintln!("Error reading from stream: {}", e);
        return;
    }

    let request = String::from_utf8_lossy(&buffer[..]);
    let map = request_parser(&request);

    let default_path = "/".to_string();
    let requested_path = map.get("Path").unwrap_or(&default_path);

    let mut path = current_dir.as_ref().clone();
    if requested_path.starts_with('/') {
        if let Some(stripped) = requested_path.strip_prefix('/') {
            path.push(stripped);
        }
    } else {
        path.push(requested_path);
    }

    let final_path = match path.canonicalize() {
        Ok(p) => {
            if p.starts_with(current_dir.as_ref()) {
                p
            } else {
                send_error_response(&mut stream, 403, "Forbidden");
                return;
            }
        }
        Err(_) => {
            send_error_response(&mut stream, 404, "Not Found");
            return;
        }
    };

    match check_is_file(&final_path) {
        FileType::Directory => {
            let page = construct_response_page(&final_path, requested_path);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                page.len(),
                page
            );
            if let Err(e) = stream.write_all(response.as_bytes()) {
                eprintln!("Failed to write response: {}", e);
            }
        }
        FileType::File => {
            if let Err(e) = send_files_response(&final_path, &mut stream) {
                eprintln!("Failed to send file response: {}", e);
            }
        }
        FileType::NotFound => {
            send_error_response(&mut stream, 404, "Not Found");
        }
    }
}

fn send_error_response(stream: &mut TcpStream, status_code: u16, reason: &str) {
    let response_body = format!("<h1>{} {}</h1>", status_code, reason);
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
        status_code,
        reason,
        response_body.len(),
        response_body
    );
    if let Err(e) = stream.write_all(response.as_bytes()) {
        eprintln!("Failed to send error response: {}", e);
    }
}

fn request_parser(request: &str) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    let first_line = request.lines().next().unwrap_or("");
    if let Some(path) = first_line.split_whitespace().nth(1) {
        if let Ok(decoded_path) = urlencoding::decode(path) {
            headers.insert("Path".to_string(), decoded_path.into_owned());
        }
    }
    headers
}

fn check_is_file(path: &Path) -> FileType {
    if !path.exists() {
        return FileType::NotFound;
    }
    if path.is_file() {
        FileType::File
    } else if path.is_dir() {
        FileType::Directory
    } else {
        FileType::NotFound
    }
}

fn construct_response_page(path: &Path, received_path: &str) -> String {
    let files = fetch_all_files(path).unwrap_or_else(|e| {
        eprintln!("Error fetching files: {}", e);
        vec![]
    });

    let mut page = String::from(
        "<!DOCTYPE html><html><head><title>File List</title><style>
            body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; margin: 2em; background-color: #f9f9f9; color: #333; }
            h1 { color: #111; }
            ul { list-style-type: none; padding: 0; }
            li { display: flex; justify-content: space-between; padding: 10px; border-bottom: 1px solid #eee; }
            li:hover { background-color: #f0f0f0; }
            a { text-decoration: none; color: #007aff; }
            .dir a { font-weight: bold; }
            .size { color: #888; font-size: 0.9em; text-align: right; }
        </style></head><body>"
    );

    page.push_str(&format!("<h1>Index of {}</h1><ul>", received_path));

    if received_path != "/" {
        let parent_path = Path::new(received_path).parent().unwrap_or(Path::new("/")).to_str().unwrap_or("/");
        page.push_str(&format!(
            "<li class='dir'><a href='{}'>.. (Parent Directory)</a><span class='size'></span></li>",
            parent_path
        ));
    }

    for file in files {
        let link_path = if received_path.ends_with('/') {
            format!("{}{}", received_path, file.name)
        } else {
            format!("{}/{}", received_path, file.name)
        };
        let class = if file.isdir { "dir" } else { "file" };
        let size_info = if file.isdir {
            "&lt;DIR&gt;".to_string()
        } else {
            format!("{} bytes", file.size)
        };

        page.push_str(&format!(
            "<li class='{}'><a href='{}'>{}</a><span class='size'>{}</span></li>",
            class, link_path, file.name, size_info
        ));
    }

    page.push_str("</ul></body></html>");
    page
}

fn fetch_all_files(path: &Path) -> Result<Vec<File>, std::io::Error> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let name = entry.file_name().into_string().unwrap_or_default();

        if metadata.is_file() {
            files.push(File {
                name,
                isdir: false,
                size: metadata.len(),
            });
        } else if metadata.is_dir() {
            files.push(File {
                name,
                isdir: true,
                size: 0,
            });
        }
    }
    Ok(files)
}

fn send_files_response(file_path: &Path, stream: &mut TcpStream) -> std::io::Result<()> {
    let metadata = std::fs::metadata(file_path)?;
    if metadata.len() > 1024 * 1024 {
        send_large_file_response(file_path, stream)
    } else {
        send_small_file_response(file_path, stream)
    }
}

fn send_small_file_response(file_path: &Path, stream: &mut TcpStream) -> std::io::Result<()> {
    let mut file = std::fs::File::open(file_path)?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n",
        contents.len()
    );

    stream.write_all(response.as_bytes())?;
    stream.write_all(&contents)?;
    Ok(())
}

fn send_large_file_response(file_path: &Path, stream: &mut TcpStream) -> std::io::Result<()> {
    let mut file = std::fs::File::open(file_path)?;
    let len = file.metadata()?.len();
    let chunk_size = 1024 * 1024;
    let mut buffer = vec![0; chunk_size];

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n",
        len
    );

    stream.write_all(response.as_bytes())?;

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        stream.write_all(&buffer[..bytes_read])?;
    }
    Ok(())
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let port = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(8123);
    let threads = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(4);

    let address = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&address)?;
    let pool = thread_pool::ThreadPool::new(threads);
    let current_dir = Arc::new(env::current_dir()?);

    println!("Server starting with {} threads.", threads);
    println!("Serving files from: {}", current_dir.display());
    println!("Listening on http://{}", address);


    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let current_dir_clone = Arc::clone(&current_dir);
                pool.execute(move || {
                    handle_client(stream, current_dir_clone);
                });
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
    }

    Ok(())
}

