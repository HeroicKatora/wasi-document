async function display(proc) {
  const out = proc.configuration.fds[3]
    ?.path_open(0, 'ex.pdf', 0, 0)
    ?.fd_obj;

  if (out == undefined) {
    throw 'Oops, process crashed before success. No output to render';
  }

  let blob = new Blob([out.file.data.buffer], { type: 'application/pdf' });
  let blobURL = URL.createObjectURL(blob);
  const el = proc.element;

  el.innerHTML =
    `<object type="application/pdf" data=${blobURL} width=1920 height=920></object>`;
}

export default display;
