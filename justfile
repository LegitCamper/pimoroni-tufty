target := "thumbv8m.main-none-eabihf"
host := `rustc -vV | sed -n 's/^host: //p'`

kernel:
    cargo build -p tufty-kernel --release

app name:
    RUSTFLAGS="-C link-arg=-pie -C relocation-model=pic" cargo build -p {{name}} --profile release-binary

apps:
    just app snake
    just app gboy
    just app chip8

test:
    cargo test -p tufty_sys -p tufty -p selection_ui --target {{host}}
    python3 -m unittest discover -s tests

check: test kernel apps
    cargo fmt --all -- --check
    just test-images

test-images:
    cargo test -p tufty_sys --target {{host}} --test built_apps -- --ignored

wifi:
    python3 tools/firmware.py
    cargo build -p tufty-kernel --release --features wifi

image root='dist/content': kernel
    mkdir -p {{root}} dist
    python3 tools/pack.py {{root}} dist/tufty-os.uf2 --kernel target/{{target}}/release/tufty-kernel
