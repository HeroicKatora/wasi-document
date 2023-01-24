A packer that adds a webpage to WASM module, making it self-hosted!

## Motivation

At the moment, Browsers can not execute WebAssembly as a native single page
app. Hopefully, this will change at some point. This post-processor allows you
to preserve the WebAssembly interpretation, unchanged, while adding an HTML
loader as a sort of polyfill instead of a native environment.

That said you could also regard it as a more general tool for WebAssembly
plugins, as a form of hypervisor. Provide a stage2 loader that emulates the
host environment within an HTML page. This could be, for instance, a WASI
environment with a file system in local storage, a simulation of a complex
native network environment, etc. Such an app can be ran natively or deployed to
a browser as a 'hardware-agnostic' alternative, from a single binary file.

## How to use it

The 'packer' acts as a filter for a WebAssembly module. It's a simple program,
if you want to explore its command line options run `wasm-as-html --help`.

```bash
# Assume you want to deploy a 'TodoMVC' app:
wasm-as-html --index-html /my/index.html /my/todomvc.js < /my/todomvc_bg.wasm > todomvc.html
```

See [examples/yew/Readme.md][examples/yew/Readme.md] for a detailed description.

Or [TodoMVC deployed on gh-pages](https://heroickatora.github.io/wasm-as-html/examples/yew/todomvc.html).

## Experimental

There's an experimental `--edit` flag. This replaces stage1 with an auto-reload
driver, which will periodically refetch the file to compare hashes. It will then
invoke the entrypoint with a response promise for the new bytes. 

## How it works

The packer takes two arguments: A WebAssembly module, and a stage 2 loader
(Javascript file) that will be given control over the page and passed a (fake)
Promise to a Response that resolves to the file's bytes. The recommended stage
2 is some web-sys implementation which passes control directly to the parsed
and instantiated module.

The technical inner workings:
* The *first* section in WASM is a custom one that parses as HTML. The trick
  from [here](https://fuzzinglabs.com/polyglot-webassembly-module-html-js-wasm/)
  implemented in a slightly more robust fashion. Note that this `stage0` is not
  of arbitrary size. Otherwise the binary encoding of the WebAssembly section's
  length will cause problems as they get parsed as HTML. This stage reads the
  first custom section with the name `wah_polyglot_stage1`.

  The stage also contains an inline link with `rel` set as `stylesheet`, in
  what is interpreted as the body. This stylesheet is applied in order to hide
  unwanted elements and display a notice in any no-JS context.

* The second section is another custom one, which is our stage1 with arbitrary
  contents. We almost immediately dispatch here from stage0 leaving behind all
  notions of HTML that we started with. We're now preparing the web page for
  the module packed as stage2. That is, loading a new skeleton with expected
  elements that the module might need to bootstrap itself.

  It reads the section named `wah_polyglot_stage1_html` as a new document
  contents, as well as the first `wah_polyglot_stage2` for the subsequent
  module. The stage will error if multiple stage2 modules are defined.

WIP: additionally, an auxiliary `.zip` file can be passed. The packer then
ensures that the result is _also_ a valid zip archive with all files intact and
such that they are accessible from the webassembly module as a custom module.
