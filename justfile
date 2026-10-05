fast:
    serpentine run --pipeline ./ci/snek/main.snek --entry-point FAST 

ci:
    serpentine run --pipeline ./ci/snek/main.snek 

book:
    serpentine run --pipeline ./ci/snek/main.snek --entry-point BOOK
    xdg-open ./target/book/index.html

bench:
    cd ./ci/benchmark/ && cargo test --release --target wasm32-unknown-unknown

bench_visual:
    cd ./ci/benchmark/ && NO_HEADLESS=1 cargo test --release --target wasm32-unknown-unknown
