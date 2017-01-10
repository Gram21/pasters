# pasters
Small paste service written in Rust

# Requirements

Currently needs postgresql installed. Configure ```.env``` to adapt to your needs.

# Usage
Clone the repository and build and run it with the following:
```
git clone https://github.com/Gram21/pasters.git
cd ./pasters
cargo run
```

This builds the debug version of the service and starts it (recommended).
The service is then accessible at [http://localhost:8000](http://localhost:8000)

A release version can be build and run with ```cargo run --release``` that then listens on port 80.

# [TODO](TODO.md)
