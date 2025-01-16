await (async function() {
  const b64 = URI_SRC.slice(URI_SRC.indexOf(",")+1);
  const buffer = new ArrayBuffer((b64.length / 4) * 3 - b64.match(/=*$/)[0].length);
  const IDX_STR = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/="
  const view = new Uint8Array(buffer);
  console.log(`Loading ${view.length} bytes of Base64 module data`);

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
})()
