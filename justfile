fast:
    serpentine run --pipeline ./ci/snek/main.snek --entry-point FAST --jobs 8

ci:
    serpentine run --pipeline ./ci/snek/main.snek --jobs 8

book:
    serpentine run --pipeline ./ci/snek/main.snek --entry-point BOOK
    xdg-open ./target/book/index.html
