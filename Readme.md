A packer that adds a webpage to WASI application, making it self-hosted!

## Motivation

At the moment, Browsers can not execute WebAssembly as a native single page
app. Maybe, this will change at some point. Or maybe we should reframe what we
expect the browser to deliver in the first place. This post-processor allows
you to preserve the WebAssembly interpretation, unchanged, while adding an HTML
loader as a sort of platform polyfill, to substitute the native environment
with a billion-dollar platform independent sandbox.

**This is not only a proof-of-concept, nor a joke**. Granted, it started out
that way but there is surprising depth of engineering that accumulated after
the initial rush of ideas. Without hyperbole, I want to explore turning it into
as much of a serious document format as PDF. The wrapper provides a robust
foundation for wrapping full applications. With the readme below, the system
can be understood fully and all its parts adjusted. Importantly, **You** can do
this.

## How to use it

The 'packer' acts as a filter for a WebAssembly module. It's a simple program,
if you want to explore its command line options run `wasm-as-html --help`.

```bash
# Assume you want to deploy a 'TodoMVC' app:
wasm-as-html --index-html /my/index.html /my/todomvc.js < /my/todomvc_bg.wasm > todomvc.html
```

See [examples/yew/Readme.md][examples/yew/Readme.md] for a detailed description.

Or [TodoMVC deployed on gh-pages](https://heroickatora.github.io/wasm-as-html/examples/yew/todomvc.html).

## Tricks related to tar compatibility

The file contents for the zip archive and bootstrapping are interpolated into
the HTML when choosing the `html+tar` target. However, due to compatibility
issues and concerns this can not happen verbatim as bytes. We prioritize the
ability to preserve the original file structure and the majority of its
execution model. Instead, the contents are encoded firstly as base64 and
secondly with additional special entries with empty filenames.

The standard `tar` implementation will skip files by complaining loudly. To get
an overview of the contained files with your standard system tools we
recommend:

```bash
tar xf $your_archive_file.html --exclude='' --to-command='base64 -d | file -'
```

You may want to modify this as a template for similar interactions with the
contained data.

## Why this specifically, or reasons against PDF

Let me offer some thoughts on the state of document pages to highlight the
considerations that lead to this specific set of choices for a document
format/or application wrapper.

I think that the format of documents in our current age is very much subpar. We
emulate 'paper' in all the wrong aspects. None of the cozy feeling are present
but all the restrictions and lack of dynamism and interaction, use of processor
power, etc. Why are we working with footnotes when the screen is wide. Why are
graphs static; why are the pictures in the first place and their raw underlying
data not accessible through the document. And; why, despite this being a known
problem for decades, is this problem still rampant?

Partially, Pdf and Adobe does not care. This is somewhat inherent in their
incentive structure. They're built on printing, most basic design decision
still were literally motivated by printers (using CMYK colors, postscript
legacy in paths). A huge part is built on 'legacy support' and nothing to gain
for reducing complexity and generalizing. And all PDF's media support is
frankly stuck in the past, consequently, and this becomes obvious in a
comparison to HTML5. No `<media>`, instead of WebGL some clusterfrick of
half-baked ideas reminiscent of Web3D (anyone remember?), and sandbox and
privacy considerations that could rival FlashPlayer.

PDF is a solution, not a platform. Try to innovate or simply think beyond the
toolbox and you're left stranded. Really we only want some standard API to
render some glyphs, developers will fill in the blanks with code. So instead
let's rebuild what we consider a document with a rendering engine that does
care a little more: *the Browser*. Don't get me wrong, these are plenty complex
as well. Yet here it is the price of comparatively quick iteration and hard
fought compatibility. (The largest threat being maybe Google's current
dominance and the risk of thus amplifying individual unfinished ideas that may
not even be their own. Yes, I do mean the refusal to seriously invest in the
technical feasibility of SPIR-V for initial WebGPU, specifically).

That said you could also regard this wrapper as not only a document engine but
a more general tool for WebAssembly plugins, as a form of hypervisor. Provide a
stage2 loader that emulates the host environment within an HTML page. This
could be, for instance, a WASI environment with a file system in local storage,
a simulation of a complex native network environment, etc. Such an app can be
ran natively or deployed to a browser as a 'hardware-agnostic' alternative,
from a single binary file.

But realistically, the temptation to shoehorn ever more features into the
'system-bindings' underneath your app and thus become very dependent on the
exact hypervisor will be hard to resist. (Look no further than Wasmer and Node
for this principle in action). This will, universally, apply to every system
where one _can_ change the underlying software. If, however, you have reason to
believe any features herein is designed to actively contribute to the effect
the *please* speak up.

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

## Overview of stages

The program inserts bootstrap sections into the WebAssembly module. These are
designed that the respective readers of other formats (html, zip, pdf)
recognize them *instead* of the WebAssembly module.

For HTML:
- A stage0 section makes the HTML parser stop after a short header, rewrites
  the main document content to a dummy page, then jumps to a module loaded from
  another section by loading that as an ES-Module. It must be the first section
  in the WebAssembly file; and it must be in a specific range of byte-lengths.
- The stage1 section takes this control and sets up a usable environment
  comparable to a single-page app. It replaces the dummy page with an initial
  page from a specially named custom section in the original module. We're free
  to run any Javascript in this module already.
- The stage2 section takes control as if some SPA module.
    - The stage2-yew case will load an application compiled, assembled, and
      packed with Yew, wasm-bindgen (or trunk if needed).
    - The stage2-wasi will now transfer control over to WASI as the main
      driver. It begins by invoking another intermediate stage program to
      control the setup of the WASI system, similar to Unix `init`.
      See [stage2](wasi-loader/Readme.md) for more.
- The default stage3 will now inspect and process the bundled zip data. This
  module takes the role of bootloader for the original module. The zip-file
  will be treated similar to an initial disk. The astute reader might instead
  use another mechanism. The author chose this as it adds transparency to the
  contents of the initial file system and its configuration files.
- The default stage4, finally, is the original WebAssembly module into which
  all these other files are packed!

For PDF [Work-In-Progress]:
- Despite the author being critical of the long-term viability of PDF, some
  people will like if they can send the resulting document such that it
  masquerades as PDF instead of HTML (e.g. a corporate report). Luckily, this
  can be arranged.
- There must be a stage0 header in the first 1kB. This will pretend to open a
  binary stream element to skip over most sections. Then an original document
  is embedded.
- As stage1, the Acrobat JavaScript API might be usable but the author does not
  particular like Acrobat's software development outcomes, in non-commercial
  settings anyways. Media and GPU embeddings are just worse and also badly
  sandboxed versions of the equivalent HTML specifications; and privacy
  nightmares. Nothing was learned from Flash. Experiment on your own.

## Experimental

There's an experimental `--edit` flag. This replaces stage1 with an auto-reload
driver, which will periodically refetch the file to compare hashes. It will
then invoke the entrypoint with a response promise for the new bytes. This may
or may not work. It can fundamentally not detect changes in any other
dependency, the browser sandbox prevents that for good reason. It is
recommended you use something like the `compile.sh` script in a watch.
