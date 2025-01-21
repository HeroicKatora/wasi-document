import * as esbuild from 'esbuild'

import { execFile } from 'node:child_process';
import fs from 'node:fs';

let cratePlugin = {
  name: 'wasm32-crate',
  setup(build) {
    // Use a reference to `Cargo.toml` to denote the target
    build.onResolve({ filter: /^cargo-wasm32:.*$/ }, args => ({
      path: args.path,
      namespace: 'cargo-crate-wasm32-ns',
    }));

    build.onResolve({ filter: /^cargo-wasi:.*$/ }, args => ({
      path: args.path,
      namespace: 'cargo-crate-wasi-ns',
    }));
    

    async function cargo_artifact(args, target) {
      let crate = args.path.replace(/cargo-wa(sm32|si):/, '');
      let exec = await new Promise((resolve, reject) => {
        execFile(
          'cargo',
          ['build', '--release', '--target', target, '-p', crate],
          (error, stdout, stderr) => {
            if (error) {
              reject(error);
            } else {
              resolve(error);
            }
          })
        });

      let contents = await fs.promises.readFile(`../target/${target}/release/${crate}.wasm`);
      let bytes = new Uint8Array(contents.buffer);

      return {
        contents: bytes,
        loader: 'binary',
      };
    }

    // Invokes the cargo build and copies the output to a binary include.
    build.onLoad({ filter: /.*/, namespace: 'cargo-crate-wasm32-ns' }, async args => {
      return await cargo_artifact(args, 'wasm32-unknown-unknown');
    })

    build.onLoad({ filter: /.*/, namespace: 'cargo-crate-wasi-ns' }, async args => {
      return await cargo_artifact(args, 'wasm32-wasi');
    })

    build.onResolve({ filter: /^wasi-config-internal-160e0777-0ea8-4def-88e7-c5297cfcb824:.*$/ }, args => ({
      path: args.path + '.js',
      namespace: 'wasi-interpreter-160e0777-0ea8-4def-88e7-c5297cfcb824',
    }))

    build.onLoad({ filter: /.*/, namespace: 'wasi-interpreter-160e0777-0ea8-4def-88e7-c5297cfcb824' }, async args => {
      const contents = await fs.promises.readFile('interpret/interpret.js');

      return {
        contents,
        resolveDir: 'node_modules',
        loader: 'js',
      };
    });
  },
}

await esbuild.build({
  entryPoints: ['interpret/interpret.js'],
  bundle: true,
  outfile: '../target/out-wasm-loader.mjs',
  format: 'esm',
  plugins: [cratePlugin],
})

// This plugin analyzes our DSL to resolve any JS dependencies needed to setup
// the environment. It then resolves to a module which exports the instructions
// and dependencies accordingly.
//
// In particular, the instruction loader can be either static or dynamic. And
// dependencies are only included if they are actually used.
let wasiInterpreterPlugin = {
  name: 'wasi-interpreter',
  setup(build) {
    build.onResolve({ filter: /^wasi-config:.*$/ }, args => ({
      path: args.path + '.mjs',
      namespace: 'wasi-interpreter-ns',
    }))

    // FIXME: we want to setup an es module with 'dynamic' requirements, based
    // on the instructions that are being used in the DSL program.
    build.onLoad({ filter: /.*/, namespace: 'wasi-interpreter-ns' }, async args => {
      // Restore the true path of configuration.
      let cfgpath = args.path.replace(/[.]mjs$/, '').replace(/^wasi-config:/, '');
      let configuration = await fs.promises.readFile(cfgpath);

      const module = await import('../target/out-wasm-loader.mjs');
      const codepoints = await module.loadInterpretedData(configuration);
      const binary = new Uint8Array(codepoints.buffer);

      // Roundtrip through numbers, not base64. `atob` work on _strings_ but we
      // do not guarantee anything like that, we just need raw bytes in any
      // order they are in. So we can't safely involve TextDecoder on the binary.
      let contents = `
      async function load_config() {
          const data = "${binary.join()}".split(",");
          return Uint8Array.from(data, c => parseInt(c, 10));
      }

      export { load_config };
      `;

      return {
        contents,
        resolveDir: 'node_modules',
        loader: 'js',
      };
    });
  },
}

await esbuild.build({
  entryPoints: ['stage2-wasi.js'],
  bundle: true,
  outfile: 'out.js',
  format: 'esm',
  plugins: [cratePlugin, wasiInterpreterPlugin],
})
