fast jobs="2":
    serpentine run --pipeline ./ci/snek/main.snek --entry-point FAST --jobs {{jobs}}

ci jobs="2":
    serpentine run --pipeline ./ci/snek/main.snek --jobs {{jobs}}

book:
    serpentine run --pipeline ./ci/snek/main.snek --entry-point BOOK
    xdg-open ./target/book/index.html

bench:
    cd ./ci/benchmark/ && cargo test --release --target wasm32-unknown-unknown

bench_visual:
    cd ./ci/benchmark/ && NO_HEADLESS=1 cargo test --release --target wasm32-unknown-unknown
