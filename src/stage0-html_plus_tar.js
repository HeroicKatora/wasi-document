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
  console.info('Wasm-As-HTML bootstrapping stage-0: started');
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
    const b64 = el.content.firstChild.textContent;
    const raw_content = b64_decode(b64);
    global.file_data[givenName] = raw_content;
  }

  const boot_wasm_bytes = global.file_data[BOOT];

  if (boot_wasm_bytes === undefined) {
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
    await module.default(boot_wasm_bytes, wasm);
  } catch (e) {
    console.fatal('Wasm-As-HTML failed to initialized', global, e);
  }
})
