# Rust HTTP File Server

A simple, fast, and multi-threaded HTTP server for serving files and directories, written in Rust.

## Description

This project is a lightweight command-line HTTP server that serves the contents of the directory it's run from. It's built to be performant by handling multiple client connections concurrently using a thread pool. It also includes basic security to prevent path traversal attacks.

## Features

-   **Directory Listing**: Provides a clean, user-friendly HTML view of directories.
-   **File Serving**: Serves any kind of file to the browser or a client like `curl`.
-   **Concurrent**: Uses a thread pool to handle multiple requests at the same time.
-   **Secure**: Prevents directory traversal attacks (e.g., requests for `../../..`).
-   **Configurable**: Allows setting the port and number of threads via command-line arguments.
-   **Efficient**: Handles large files by streaming them in chunks.

## Prerequisites

You need to have the Rust programming language and its package manager, Cargo, installed. You can get them from [rustup.rs](https://rustup.rs/).

## Building and Running

1.  **Clone the repository:**
    ```sh
    git clone <your-repository-url>
    cd <repository-name>
    ```

2.  **Build the project for release:**
    ```sh
    cargo build --release
    ```
    The executable will be located at `target/release/cmd`.

3.  **Run the server:**

    *   **With default settings** (Port: `8123`, Threads: `4`):
        ```sh
        ./target/release/file_server
        ```

    *   **With a custom port** (e.g., port `8080`):
        ```sh
        ./target/release/file_server 8080
        ```

    *   **With a custom port and thread count** (e.g., port `8080`, `8` threads):
        ```sh
        ./target/release/file_server 8080 8
        ```

    Once running, open your web browser and navigate to `http://127.0.0.1:8123` (or your custom port).

## Dependencies

This project uses the following external crate:
-   [`urlencoding`](https://crates.io/crates/urlencoding): For decoding URL-encoded characters in request paths.

Cargo will automatically handle the installation of this dependency when you build the project.
