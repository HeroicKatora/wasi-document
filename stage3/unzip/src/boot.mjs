export default async function(configuration) {
  /* Problem statement:
   * We'd like to solve the problem of exporting our current WASI for use by
   * wasm-bindgen. It is not currently supported to pass such additional
   * imports as a parameter to the init function of wasm-bindgen. Instead, the
   * module generated looks like so:
   *
   *     import * as __wbg_star0 from 'wasi_snapshot_preview1';
   *     // etc.
   *     imports['wasi_snapshot_preview1'] = __wbg_star0;
   *
   * Okay. So can we setup such that the above `wasi_snapshot_preview1` module
   * refers to some shim that we control? Not so easy. We can not simply create
   * an importmap; we're already running in Js context and it's forbidden to
   * modify after that (with some funny interaction when rewriting the whole
   * document where warnings are swallowed?). See `__not_working_via_importmap`.
   *
   * Instead, we will hackishly rewrite the bindgen import if we're asked to.
   * Create a shim module that exports the wasi objects' import map, and
   * communicate with the shim module via a global for lack of better ideas. I
   * don't like that we can not reverse this, the module is cached, but oh
   * well. Let's hope for wasm-bindgen to cave at some point. Or the browser
   * but 'Chromium does not have the bandwidth' to implement the dynamic remap
   * feature already in much smaller products. And apparently that is the
   * motivation not to move forward in WICG. Just ____ off. When talking about
   * Chrome monopoly leading to bad outcomes, this is one. And no one in
   * particular is at fault of course.
   */
  async function reap_into_inner_text(proc) {
    const [stdin, stdout, stderr] = proc.configuration.fds;
    proc.element.innerText = new TextDecoder().decode(stdout.file.data);
    proc.element.title = new TextDecoder().decode(stderr.file.data);
  }

  console.log('Reached stage3 successfully', configuration);
  const wasm = configuration.wasm_module;

  let newWasi = new configuration.WASI(configuration.args, configuration.env, configuration.fds);
  document.__wah_wasi_imports = newWasi.wasiImport;

  const kernel_bindings = WebAssembly.Module.customSections(wasm, 'wah_polyglot_wasm_bindgen');

  // A kernel module is any Module which exposes a default export.that conforms
  // to our call interface. It will get passed a promise to the wasmblob
  // response of its process image and should be an awaitable that resolves to
  // the exports from the module. Simplistically this could be the `exports`
  // attribute from the `Instance` itself.
  let kernel_module = undefined;
  if (kernel_bindings.length > 0) {
    // Create a module that the kernel can `import` via ECMA semantics. This
    // enables such kernel modules to be independent from our target. In fact,
    // we do expect them to be created via Rust's `wasm-bindgen` for instance.
    let testmodule = Object.keys(document.__wah_wasi_imports)
      .map((name, _) => `export const ${name} = document.__wah_wasi_imports.${name};`)
      .join('\n');
    let wasi_blob = new Blob([testmodule], { type: 'application/javascript' });
    let objecturl = URL.createObjectURL(wasi_blob);

    // FIXME: should be an import map where `wasi_snapshot_preview1` is an
    // alias for our just created object URL module.
    const wbg_source = new TextDecoder().decode(kernel_bindings[0])
      .replace('wasi_snapshot_preview1', objecturl);

    let wbg_blob = new Blob([wbg_source], { type: 'application/javascript' });
    let wbg_url = URL.createObjectURL(wbg_blob);
    kernel_module = await import(wbg_url);
  }

  const index_html = WebAssembly.Module.customSections(wasm, 'wah_polyglot_stage1_html');
  if (index_html.length > 0) {
    document.documentElement.innerHTML = (new TextDecoder().decode(index_html[0]));
  }

  const rootdir = configuration.fds[3];
  configuration.fds[0] = rootdir.path_open(0, "proc/0/fd/0", 0, 1).fd_obj;
  configuration.fds[1] = rootdir.path_open(0, "proc/0/fd/1", 0, 1).fd_obj;
  configuration.fds[2] = rootdir.path_open(0, "proc/0/fd/2", 0, 1).fd_obj;
  configuration.args.length = 0;

  const input_decoder = new TextDecoder('utf-8');
  const assign_arguments = (path, push_with, cname) => {
    cname = cname || 'cmdline';
    let cmdline = undefined;
    if (cmdline = rootdir.path_open(0, path, 0, 1).fd_obj) {
      let data = cmdline.file.data;
      let nul_split = -1;
      while ((nul_split = data.indexOf(0)) >= 0) {
        const arg = data.subarray(0, nul_split);
        push_with(input_decoder.decode(arg));
        data = data.subarray(nul_split + 1);
      }

      push_with(input_decoder.decode(data));
    } else {
      console.log('No initial', cname);
    }
  }

  assign_arguments("proc/0/cmdline", (e) => configuration.args.push(e), "cmdline");
  assign_arguments("proc/0/environ", (e) => configuration.env.push(e), "environ");

  let reaper = [];

  try {
    console.log('Dispatch stage3 into init', configuration);

    // The init process controls the whole body in the end.
    reaper.push({
      configuration: configuration,
      // FIXME: hardcoded but permissible?
      post_module: reap_into_inner_text,
      // FIXME: hardcoded, should be configurable
      override_file: 'proc/0/display.mjs',
      // The element context.
      //
      // FIXME: hardcoded, should be configurable. Also if we launch multiple
      // process instances concurrently then they are configured by finding a
      // number of `<template>` elements that contain instructions for a
      // derived configuration in that shared environment. Then the context is
      // that element itself, allowing it to be replaced with the actual
      // rendering intent.
      //
      // FIXME: but replacement should also be possible early if we want to
      // avoid flicker.
      element: document.getElementsByTagName('body')[0],
    });

    var source_headers = {};
    const wasmblob = new Blob([configuration.wasm], { type: 'application/wasm' });

    if (kernel_module !== undefined) {
      const ret = await kernel_module.default(Promise.resolve(new Response(wasmblob, {
        'headers': source_headers,
      })));

      await newWasi.start({ 'exports': ret });
    } else {
      const imports = { 'wasi_snapshot_preview1': newWasi.wasiImport };
      const instance = await WebAssembly.instantiate(wasm, imports);
      await newWasi.start({ 'exports': instance.exports });
    }
  } catch (e) {
    if (typeof(e) == 'string' && e == 'exit with exit code 0') {} else {
      console.dir(typeof(e), e);
      console.log('at ', e.fileName, e.lineNumber, e.columnNumber);
      console.log(e.stack);
      configuration.fallback_shell(configuration, e);
    }
  } finally {
    const [stdin, stdout, stderr] = configuration.fds;
    console.log('Result(stdin )', new TextDecoder().decode(stdin.file.data));
    console.log('Result(stdout)', new TextDecoder().decode(stdout.file.data));
    console.log('Result(stderr)', new TextDecoder().decode(stderr.file.data));
  }

  let display = await Promise.allSettled(reaper.map(async function(proc) {
    const override_file = proc.configuration.fds[3]
      ?.path_open(0, proc.override_file, 0, 0)
      ?.fd_obj;

    let post_handler = proc.post_module;

    if (override_file) {
      let blob = new Blob([override_file.file.data.buffer], { type: 'application/javascript' });
      let blobURL = URL.createObjectURL(blob);
      post_handler = (await import(blobURL)).default;
    }

    // FIXME: unclean. We expose all our WASI internals here. The exact classes
    // etc. While this may sometimes be necessary for precise control and the
    // layer is, after all, below us and thus our 'target platform' it would be
    // much nicer if we had a more fine-grained decision about the exposed
    // object where everything / most is opt-in and explicitly requested.
    return await post_handler(proc);
  }));

  let have_an_error = undefined;
  if ((have_an_error = display.filter(el => el.reason !== undefined)).length > 0) {
    configuration.fallback_shell(configuration, have_an_error);
  }
}

// The concept that did not work, importmap must not be modified/added from a script.
function __not_working_via_importmap(objecturl) {
  if (!(HTMLScriptElement.supports?.("importmap"))) {
    throw "Browser must support import maps.";
  }

  const importmap = JSON.stringify({
    "imports": {
      "testing": objecturl,
    },
  });

  const d = document.createElement('script');
  d.type = 'importmap';
  d.innerText = importmap;

  document.documentElement.innerHTML = `<head></head>`;
  document.head.appendChild(d);
}
