The `uutils` shell enviroment, prepared to run on web/wasi. Run in this directory:

```bash
cargo install --root . uutils --target wasm32-wasip1 --git https://github.com/HeroicKatora/uutils
```

This will leave a file `bin/coreutils.wasm` beneath this directory. That file
is a poly-binary that implements all the basic system binaries supported by
uutils, based on the program name it is called with. The prepare the init
environment via the file system found by the loader:

```bash
# Replace the init process's argument list as appropriate
echo -ne '/bin/cksum -a sha256 /etc/wasi-release' | tr ' ' '\00' > examples/uutils/root/proc/0/cmdline
# Replace the environment list as appropriate
echo -ne 'CWD=/' | tr ' ' '\00' > examples/uutils/root/proc/0/environ
```

Finally, bundle it up in the project directory:

```bash
cargo build --release -p unzip --target wasm32-wasip1

cargo run --\
 --target html+tar\
 --index-html examples/uutils/index.html wasi-loader/out.js\
 --stage3 target/wasm32-wasip1/release/unzip.wasm\
 --root-fs examples/uutils/root/\
 < examples/uutils/bin/coreutils.wasm  > /tmp/out.html
```

Now open `/tmp/out.html` in your browser!
