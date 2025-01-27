async function init(bytes, wasm, wasi_root_fs) {
  let index_html = WebAssembly.Module.customSections(wasm, 'wah_polyglot_stage1_html');

  if (index_html.length) {
    document.documentElement.innerHTML = (new TextDecoder().decode(index_html[0]));
  } else {
    document.getElementById('stage0_error').innerText = '';
  }

  let stage2 = WebAssembly.Module.customSections(wasm, 'wah_polyglot_stage2');
  if (!stage2.length) {
    throw 'Found no application data. Please check distribution.';
  }
  if (!stage2.length > 1) {
    throw 'Found duplicate application data. Please check distribution.';
  }

  /* This is the wasm-bindgen flavor.
       It is one module with an default export (`init`). The exported
       function can take a Promise to a Response object that resolves to the WASM module.
       Since we have it already we just create a synthetic response.
   **/
  let blob = new Blob([stage2[0]], { type: 'application/javascript' });
  let blobURL = URL.createObjectURL(blob);
  let stage2_module = (await import(blobURL));

  /** wasm-bindgen: creates one 
  */
  let wasmblob = new Blob([bytes], { type: 'application/wasm' });
  stage2_module.default({
    module_or_path: Promise.resolve(new Response(wasmblob)),
    wasi_root_fs: wasi_root_fs,
  });
}

export default init;
