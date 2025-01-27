/** An entry/on-load script to 'boots' a browser Javascript environment into
 * the packed bootstrapping format defined by the wasm-as-html project. We
 * expect to be started in an HTML page that was prepared with the `html+tar`
 * polyglot structure. That is there should be a number of `<template>`
 * elements which escape tar headers and contents so as not to be interpreted
 * by the HTML structure. Our task is to gather them up, undo the encoding used
 * to hide them here and make the resulting file tree available for further
 * processing. Then we inspect that tree for a special boot file that defines a
 * stage-1 payload WASM whose definition are shared with other stage-0 encoding
 * entry points and whose contents we interpret accordingly to further dispatch
 * into our bootstrap process. (The stage-1 then sets up for executing a
 * WebAssembly module, which will regularize the environment to execute the
 * original module).
 *
 * The code here is quite self-contained with the main piece being an inlined
 * base64 decoder that is actually _correct_ for all inputs we throw at it.
 */

// State object, introspectable for now.
let __wah_stage0_global = {};
const BOOT = 'boot/wah-init.wasm';

function b64_decode(b64) {
  const buffer = new ArrayBuffer((b64.length / 4) * 3 - b64.match(/=*$/)[0].length);
  const IDX_STR = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/="
  const view = new Uint8Array(buffer);

  let i = 0;
  let j = 0;
  for (; j < b64.length;) {
    let a = IDX_STR.indexOf(b64.charAt(j++));
    let b = IDX_STR.indexOf(b64.charAt(j++));
    let c = IDX_STR.indexOf(b64.charAt(j++));
    let d = IDX_STR.indexOf(b64.charAt(j++));

    view[i++] = (a << 2) | (b >> 4);
    if (c < 64) view[i++] = ((b & 0xf) << 4) | (c >> 2);
    if (d < 64) view[i++] = ((c & 0x3) << 6) | (d >> 0);
  }

  return view;
}

window.addEventListener('load', async function() {
  console.debug('Wasm-As-HTML bootstrapping stage-0: started');
  const dataElements = document.getElementsByClassName('wah_polyglot_data');

  let global = __wah_stage0_global;
  global.file_elements = {};
  global.file_data = {};

  for (let el of dataElements) {
    const givenName = el.getAttribute('_wahtml_id')
      ?.replaceAll(String.fromCodePoint(0xfffd), '')
      ?.replaceAll(String.fromCodePoint(0), '');

    if (givenName === null) {
      continue;
    }

    global.file_elements[givenName] = el;
    // NOTE: usually we `firstChild.textContent`. But for reasons unknown to me
    // at the moment of writing this truncates the resulting string to a clean
    // 1<<16 bytes instead of retaining the full encoding; in Chromium browsers
    // but not in Firefox.
    // NOTE: now being more knowledgable, it's probably that the content
    // already is a pure text node. So its first child attribute is probably
    // synthetic and there's some encoding roundtrip which mangles it. Eh. This
    // is fine if it works and we do control the encoding side as well.
    const b64 = el.content.textContent;
    const raw_content = b64_decode(b64);
    global.file_data[givenName] = raw_content;
  }

  const boot_wasm_bytes = global.file_data[BOOT];

  if (boot_wasm_bytes === undefined) {
    console.debug('Wasm-As-HTML bootstrapping stage-0: no handoff to boot, done');
    return;
  }

  let wasm = await WebAssembly.compileStreaming(
    new Response(boot_wasm_bytes, { headers: { 'content-type': 'application/wasm' }})
  );

  try {
    let stage1 = WebAssembly.Module.customSections(wasm, 'wah_polyglot_stage1')[0];
    let blob = new Blob([stage1], { type: 'application/javascript' });
    let blobURL = URL.createObjectURL(blob);
    let module = (await import(blobURL));
    console.debug('Wasm-As-HTML bootstrapping stage-0: handoff');
    await module.default(boot_wasm_bytes, wasm, global.file_data);
  } catch (e) {
    console.error('Wasm-As-HTML failed to initialized', global, e);
  }
})
