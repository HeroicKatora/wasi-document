import { WASI, File, OpenFile, Directory, PreopenDirectory } from "@bjorn3/browser_wasi_shim";
// This include is synthesized by `build.js:wasiInterpreterPlugin`.
import { load_config } from 'wasi-config:config.toml'

async function fallback_shell(configuration, error) {
  document.documentElement.innerHTML = `<p>Missing boot exec</p>`;
  if (error !== undefined) {
    document.documentElement.innerHTML = `<p>Error: ${error}</p>`;
  }

  document.documentElement.innerHTML += `<p>WAH rescue shell</p>`;
  // FIXME: here we would search for a special section, that can _optionally_
  // added if the file size is not too consequential. This would then contain
  // an actual shell compiled as a separate single-module ESM, a new stage2/SPA
  // basically.
  const rootfs = configuration.fds[3];

  function mkDirElement(dir) {
    const list = document.createElement('ul');
    for (const [name, entry] of Object.entries(dir.contents)) {
      const el = document.createElement('li');
      el.innerText = name;
      if (entry instanceof Directory) {
        const sublist = mkDirElement(entry);
        el.appendChild(sublist);
      } else {
        el.innerText = '';
        const btn = document.createElement('a');
        const blob = new Blob([entry.data]);
        btn.innerText = name;
        btn.download = name;
        btn.href = URL.createObjectURL(blob);
        el.appendChild(btn);
      }

      list.appendChild(el);
    }
    return list;
  };

  document.documentElement.innerHTML += `<p>Filesystem: </p>`;
  document.documentElement.appendChild(mkDirElement(rootfs.dir));

  console.log(error);
}

async function mount({ module_or_path, wasi_root_fs }) {
  const response = await module_or_path;
  const [body_wasm, body_file] = response.body.tee();

  let wasm = await WebAssembly.compileStreaming(new Response(body_wasm, {
      'status': response.status,
      'statusText': response.statusText,
      'headers': response.headers,
    }));

  // Convert a body (compatible with `Response`) into an array buffer of its
  // contents by re-emulating an existing previous response reception.
  const body_to_array_buffer = async function(response, body_file) {
    const newbody = new Response(body_file, {
      'status': response.status,
      'statusText': response.statusText,
      'headers': response.headers,
    });

    return await newbody.arrayBuffer();
  };

  var configuration = {
    args: ["exe"],
    env: [],
    fds: [],
    wasm: await body_to_array_buffer(response, body_file),
    wasm_module: wasm,
  };

  let wah_wasi_config_data = WebAssembly.Module.customSections(wasm, 'wah_wasi_config');
  wah_wasi_config_data.unshift(new TextEncoder('utf-8').encode('{}'));

  if (wah_wasi_config_data.length > 1) {
    // We can not handle this. Okay, granted, we could somehow put it into the
    // configuration object and let the script below handle it. It could be
    // said that I have not decided how to handle it. The section is ignored
    // anyways for now.
    throw `Multiple configuration sections 'wah_wasi_config' detected`;
  } else {
    const instr_debugging = console.log;
    /* Optional: we could pre-execute this on the config data, thus yielding
     * the `output` instructions.
     **/
    let raw_configuration = await load_config(wah_wasi_config_data[0]);

    let data = new Uint8Array(raw_configuration.buffer);
    let instruction_stream = new Uint32Array(raw_configuration.buffer);
    var iptr = 0;

    // The configuration output is 'script' in a simple, static assignment
    // scripting language. We have objects and each instruction calls one of
    // them with some arguments.
    //
    // Why are we having a script here, and not eval'ing Js? Well.. For once I
    // like have a rather small but configurable section. Js on the other hand
    // would be quite verbose. If in doubt, we have a `function` constructor as
    // an escape hatch?
    const ops = [
      /* 0: the configuration object */
      configuration,
      /* 1: skip */ 
      (cnt) => {
        instr_debugging(`skip ${cnt} to ${iptr+cnt}`);
        return iptr += cnt;
      },
      /* 2: string */
      (ptr, len) => {
        instr_debugging(`decode ${ptr} to ${ptr+len}`);
        return new TextDecoder('utf-8').decode(data.subarray(ptr, ptr+len));
      },
      /* 3: json */
      (ptr, len) => {
        instr_debugging(`json ${ptr} to ${ptr+len}`);
        return JSON.parse(data.subarray(ptr, ptr+len));
      },
      /* 4: integer const */
      (c) => {
        instr_debugging(`const ${c}`);
        return c;
      },
      /* 5: array */
      (ptr, len) => {
        instr_debugging(`array ${ptr} to ${ptr+len}`);
        return data.subarray(ptr, ptr+len);
      },
      /* 6: get */
      (from, idx) => {
        instr_debugging('get', from, ops[idx], (ops[from])[ops[idx]]);
        return (ops[from])[ops[idx]];
      },
      /* 7: set */
      (into, idx, what) => {
        instr_debugging('set', into, ops[idx], ops[what]);
        return (ops[into])[ops[idx]] = ops[what];
      },
      /* 8: File */
      (what) => {
        instr_debugging('file', ops[what]);
        return new File(ops[what]);
      },
      /* 9: Directory */
      (what) => {
        instr_debugging('directory', ops[what]);
        return new Directory(ops[what]);
      },
      /* 10: PreopenDirectory */
      (where, what) => {
        instr_debugging('preopen directory', ops[where], ops[what]);
        return new PreopenDirectory(ops[where], ops[what]);
      },
      /* 11: Directory.open */
      (dir, im_flags, path, im_oflags) => {
        instr_debugging('diropen', dir, im_flags, ops[path], im_oflags);
        return ops[dir].path_open(im_flags, ops[path], im_oflags).fd_obj;
      },
      /* 12: OpenFile */
      (what) => {
        instr_debugging('fileopen', ops[what]);
        return new OpenFile(ops[what]);
      },
      /* 13: section */ // FIXME: maybe pass the module itself explicitly?
      // Do we want to support compiling modules already at this point?
      (what) => {
        instr_debugging('wasm', ops[what]);
        return WebAssembly.Module.customSections(wasm, ops[what]);
      },
      /* 14: no-op */
      function() {
        instr_debugging('noop', arguments);
        return {};
      },
      /* 15: function */
      (what) => {
        instr_debugging('function', ops[what]);
        return new Function(ops[what]);
      },
    ];

    ops[255] = undefined;
    document.documentElement.textContent = '\n';

    try {
      while (iptr < instruction_stream.length) {
        let fn_ = ops[instruction_stream.at(iptr)];
        let acnt = instruction_stream.at(iptr+1);
        let args = instruction_stream.subarray(iptr+2, iptr+2+acnt);

        ops.push(fn_.apply(null, args));
        iptr += 2 + acnt;
      }
    } catch (e) {
      console.log('Instructions failed', e);
      console.log(ops);
      document.documentElement.textContent += '\nOops: ';
      document.documentElement.textContent += '\nError: '+e;
    }

    document.documentElement.textContent += `Initialized towards stage3 in ${ops.length-256} steps`;
  }

  let args = configuration.args;
  let env = configuration.env;
  let fds = configuration.fds;
  let filesystem = configuration.fds[3];
  configuration.WASI = WASI;

  if (wasi_root_fs) {
    // The given layer will be underlaid the inputs to the boot archive extractor.
    for (const [key, value] of Object.entries(wasi_root_fs)) {
      let dirs = key.split('/');
      const file = dirs.pop();

      let basedir = filesystem;
      for (let dir of dirs) {
        // NOTE: should succeed with create_directory if we set OFLAGS_CREAT as
        // well but some versions of the shim handle this situation badly. So
        // do this in steps.
        let reldir = basedir.path_open(0, dir, WASI.OFLAGS_DIRECTORY);

        if (!reldir.fd_obj) {
          basedir.path_create_directory(dir);
          reldir = basedir.path_open(0, dir, WASI.OFLAGS_DIRECTORY);
        }

        if (!reldir.fd_obj) {
          console.log('Did not create..', key, dir, reldir, filesystem);
          break;
        }

        basedir = reldir.fd_obj;
      }

      // Open read-write with creation flags.
      const maybefd = basedir.path_open(0, file, 1, 1);

      // Error handling, supposing this signals ENOSUP just as well.
      if (!maybefd.fd_obj) {
        console.log('Did not write..', file, key, maybefd, basedir);
        continue;
      }

      const data_array = new Uint8Array(value);
      maybefd.fd_obj.file.data = data_array;
    }
  }


  configuration.wasi = new WASI(args, env, fds);
  // The primary is setup as the executable image of proc/0/exe (initially the stage4).
  const boot_exe = filesystem.path_open(0, "boot/init", 0).fd_obj;

  // FIXME: error handling?
  // If this is still something then let's replace.
  const primary_wasm = await WebAssembly.compileStreaming(new Response(
    new Blob([boot_exe.file.data.buffer], { type: 'application/javascript' }),
    { 'headers': response.headers }));

  let inst = await WebAssembly.instantiate(primary_wasm, {
    "wasi_snapshot_preview1": configuration.wasi.wasiImport,
  });

  const [stdin, stdout, stderr] = configuration.fds;

  try {
    try {
      configuration.wasi.start(inst);
    } catch (e) {
      document.documentElement.innerHTML += `<p>Failed initialization: ${e}</p>`;
      document.documentElement.innerHTML += `<p>Result(stdout) ${new TextDecoder().decode(stdout.file.data)}</p>`;
      document.documentElement.innerHTML += `<p>Result(stderr) ${new TextDecoder().decode(stderr.file.data)}</p>`;
    }
  } finally {
    console.log('Result(stdin )', new TextDecoder().decode(stdin.file.data));
    console.log('Result(stdout)', new TextDecoder().decode(stdout.file.data));
    console.log('Result(stderr)', new TextDecoder().decode(stderr.file.data));
  }

  let module = filesystem.path_open(0, "boot/index.mjs", 0).fd_obj;
  if (module == null) {
    return await fallback_shell(configuration);
  }

  let blob = new Blob([module.file.data.buffer], { type: 'application/javascript' });
  let blobURL = URL.createObjectURL(blob);
  let stage3_module = (await import(blobURL));
  configuration.fallback_shell = fallback_shell;

  console.log('executing boot module');
  try {
    await stage3_module.default(configuration);
  } catch (e) {
    await fallback_shell(configuration, e);
  }
}

export default mount;
