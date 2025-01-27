use std::{io::Read, io::Write, path::PathBuf};

use clap::Parser;
#[cfg(feature = "target-html+tar")]
mod dom;
mod error;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = Args::parse();

    let stage_2 = std::fs::read(&args.stage_2)?;
    let wasm = match &args.wasm {
        None => {
            let mut stdin = std::io::stdin();
            let mut data = vec![];
            stdin.read_to_end(&mut data)?;
            data
        }
        Some(path) => std::fs::read(path)?,
    };

    let parser = wasmparser::Parser::default();
    let mut encoder = wasm_encoder::Module::new();

    encoder.section(&wasm_encoder::CustomSection {
        name: "wah_polyglot_stage0",
        // Html designed to terminate processing into further WASM sections. This is the only
        // section that needs to be placed specifically at the start. All other sections are then
        // parsed from the module.
        data: include_bytes!("stage0-wasm.html"),
    });

    // The actual (document) loader that prepares inputs and control for stage 2.
    encoder.section(&wasm_encoder::CustomSection {
        name: "wah_polyglot_stage1",
        data: if args.edit {
            assert!(std::env::var_os("WAH_POLYGLOT_EXPERIMENTAL").is_some());
            include_bytes!("stage1-edit.js")
        } else {
            include_bytes!("stage1.js")
        },
    });

    if let Some(index) = &args.index_html {
        let index_html = std::fs::read(index)?;

        encoder.section(&wasm_encoder::CustomSection {
            name: "wah_polyglot_stage1_html",
            data: &index_html,
        });
    }

    encoder.section(&wasm_encoder::CustomSection {
        name: "wah_polyglot_stage2",
        data: &stage_2,
    });

    for section in parser.parse_all(&wasm) {
        if let Some((id, data_range)) = section.map_err(parse_err)?.as_section() {
            encoder.section(&wasm_encoder::RawSection {
                id,
                data: &wasm[data_range],
            });
        }
    }

    // NOTE: suppose that all third-stage loaders user the WASM module sections as the main source
    // of truth for their init variant. Of course this need not hold and that's when this and the
    // extra section loop should be re-examined.
    if let Some(stage3) = &args.stage3 {
        args.extra_section.push(ExtraSection {
            name: "wah_polyglot_stage3".to_string(),
            from_file: stage3.clone(),
        });
    }

    for extra in &args.extra_section {
        encoder.section(&wasm_encoder::CustomSection {
            name: &extra.name,
            data: &std::fs::read(&extra.from_file)?,
        });
    }

    let needs_zip_section = !matches!(args.target, Target::HtmlPlusTar);

    if needs_zip_section {
        if let Some(zip_file) = &args.zip {
            let zip_data = std::fs::read(zip_file)?;
            let name = args
                .zip_section_name
                .as_deref()
                .unwrap_or("wah_polyglot_stage2_data");

            encoder.section(&wasm_encoder::CustomSection {
                name,
                data: &zip_data,
            });
        }
    }

    let wasm = match args.target {
        Target::WasmPlusHtml => encoder.finish(),
        Target::Html => {
            use base64::{display::Base64Display, engine::general_purpose};
            let wasm = encoder.finish();
            let template = include_str!("stage0-html.html");

            // To include our WebAssembly module as data, we need to massage the data into an HTML
            // compatible form. In the end, access to it as an ArrayBuffer is required. The pure
            // `fetch` is sometimes limited by the browser so maybe that's a problem? But it is the
            // most efficient base64 decoder we have. And it is **correct**.
            //
            // <https://stackoverflow.com/questions/21797299/convert-base64-string-to-arraybuffer>
            // There answers are mostly bad, and confidently incorrect.
            let wasm = Base64Display::new(&wasm, &general_purpose::STANDARD);
            let data_uri = format!("data:application/octet-stream;base64,{wasm}");
            let data_uri_constructor = format!("{data_uri}");
            let with_data = template.replace(
                "__REPLACE_THIS_WITH_WASM_AS_A_DATA_URI__",
                &data_uri_constructor,
            );

            // 16 MB is generally okay..
            let loaded = if data_uri.len().ilog2() < 24 {
                // Nothing to do..
                with_data.replace(
                    "__REPLACE_THIS_WITH_URI_LOADER__",
                    "await (async function() {
                    let doc = await fetch(URI_SRC);
                    return await doc.arrayBuffer();
                })()",
                )
            } else if data_uri.len().ilog2() < 31 {
                // For larger module (scarily large) we need a different strategy that is not yet
                // implemented here. In particular, Firefox makes a 32MB restriction on the size of
                // a DataUrl which seems to be enforced when `fetch` is called (which converts it
                // into an URL object? Not quite sure where the actual restriction is placed).
                // We do
                //
                // FIXME: Evaluate ReadableByteStream for chunk-based yields <https://developer.mozilla.org/en-US/docs/Web/API/ReadableByteStreamController>
                //
                with_data.replace(
                    "__REPLACE_THIS_WITH_URI_LOADER__",
                    include_str!("stage-snippet-load-URI-b64.js"),
                )
            } else {
                // This is too large for most String implementations..
                panic!("The `html` target does not support modules larger than 2GB.");
            };

            loaded.into()
        }
        #[cfg(not(feature = "target-html+tar"))]
        Target::HtmlPlusTar => {
            return Err(error::UnsupportedFeatureError {
                what_to_use: "target-html+tar".into(),
                feature: "target-html+tar".into(),
            })?;
        }
        #[cfg(feature = "target-html+tar")]
        Target::HtmlPlusTar => {
            let Some(template) = args.index_html else {
                panic!("The `html+tar` target embeds into the index HTML.");
            };

            let source = std::fs::read_to_string(&template)?;
            let mut source = dom::SourceDocument::new(&source);
            let binary_wasm = encoder.finish();
            let source_script = include_bytes!("stage0-html_plus_tar.js");

            let structure = source.prepare_tar_structure()?;

            let mut engine = html_and_tar::TarEngine::default();
            let mut seq_of_bytes: Vec<&[u8]> = vec![];

            let mut head_span = source.span(structure.html_tag);
            head_span.end = head_span.start + structure.html_insertion_point;
            head_span.start = 0;

            let head = &source[head_span];
            let where_to_insert = source.span(structure.insertion_tag);
            let where_to_enter = source.span(structure.stage0);

            assert!(where_to_insert.end < where_to_enter.start);

            let init = engine.start_of_file(head.as_bytes(), where_to_insert.start);
            seq_of_bytes.push(init.header.as_bytes());
            seq_of_bytes.push(init.extra.as_slice());
            seq_of_bytes.push(source[init.consumed..where_to_insert.start].as_bytes());

            let mut pushed_data = vec![];

            pushed_data.push(engine.escaped_insert_base64(html_and_tar::Entry {
                name: "boot/wah-init.wasm",
                data: &binary_wasm,
            }));

            if let Some(zip) = args.zip {
                let file = std::fs::File::open(zip)?;
                let mut archive = zip::read::ZipArchive::new(file)?;

                for idx in 0..archive.len() {
                    let mut file = archive.by_index(idx)?;

                    let Some(name) = file.enclosed_name() else {
                        continue;
                    };

                    let Some(name) = name.to_str() else {
                        continue;
                    };

                    let mut data = vec![];
                    file.read_to_end(&mut data)?;

                    let entry =
                        engine.escaped_continue_base64(html_and_tar::Entry { name, data: &data });

                    pushed_data.push(entry);
                }
            }

            if let Some(root) = args.root_fs {
                let iter = walkdir::WalkDir::new(&root).same_file_system(true);

                for entry in iter {
                    let entry = entry?;

                    let full_path = entry.path();
                    let meta = entry.metadata()?;

                    let Ok(path) = full_path.strip_prefix(&root) else {
                        continue;
                    };

                    let Some(name) = path.to_str() else {
                        continue;
                    };

                    if !meta.is_file() {
                        continue;
                    }

                    let data = std::fs::read(&full_path)?;

                    let entry =
                        engine.escaped_continue_base64(html_and_tar::Entry { name, data: &data });

                    pushed_data.push(entry);
                }
            }

            for data in &pushed_data {
                seq_of_bytes.push(data.padding);
                seq_of_bytes.push(data.header.as_bytes());
                seq_of_bytes.push(data.file.as_bytes());
                seq_of_bytes.push(data.data.as_slice());
            }

            // FIXME: not sure if we should just do the open-end thing instead of EOF..

            let eof = engine.escaped_eof();
            seq_of_bytes.push(eof.padding);
            seq_of_bytes.push(eof.header.as_bytes());
            seq_of_bytes.push(eof.file.as_bytes());
            seq_of_bytes.push(eof.data.as_slice());

            seq_of_bytes.push(source[where_to_insert.end..where_to_enter.start].as_bytes());
            seq_of_bytes.push(b"<script>");
            seq_of_bytes.push(source_script);
            seq_of_bytes.push(b"</script>");
            seq_of_bytes.push(source[where_to_enter.end..].as_bytes());

            seq_of_bytes.join(&b""[..])
        }
    };

    match &args.out {
        None => {
            let mut stdout = std::io::stdout();
            stdout.write_all(&wasm)?;
        }
        Some(path) => {
            std::fs::write(path, &wasm)?;
        }
    }

    Ok(())
}

fn parse_err(_: wasmparser::BinaryReaderError) -> std::io::Error {
    todo!()
}

#[derive(Clone, Debug)]
struct ExtraSection {
    name: String,
    from_file: PathBuf,
}

#[derive(Parser)]
struct Args {
    // Positional arguments
    /// The stage 2 loader payload, a JS module.
    ///
    /// Stage 0 refers to the necessary inline script block to take control of HTML processing,
    /// stage 1 to the built-in jump pad implemented as a separate Javascript custom section.
    ///
    /// The stage 2 payload is your module that gains control of execution and is invoked with a
    /// fake request that resolves to full WASM module, after the page has been replaced with the
    /// indicated `index.html`. The stage 1 will call its default export as
    ///
    /// stage2_module.default(Promise.resolve(new Response(wasmblob)))
    #[arg(name = "STAGE2_JS")]
    stage_2: PathBuf,
    /// The web assembly module to embed ourselves in, default stdin.
    wasm: Option<PathBuf>,

    // Options.
    /// A file to write the module to, default stdout.
    #[arg(short, long)]
    out: Option<PathBuf>,
    /// An HTML page to use when invoking the loader. Setup by the stage 1 loader. Defaults to an
    /// empty page that hides some garbage from processing the WASM module header.
    #[arg(short, long)]
    index_html: Option<PathBuf>,
    /// A zip file to attach.
    ///
    /// This file is added as a final section of the module (so its central archive is within the
    /// last 512 bytes). Note that some targets do not support the zip file. Provide the files with
    /// a root fs instead, the archive will be unpacked accordingly.
    #[arg(short, long = "trailing-zip", alias = "zip")]
    zip: Option<PathBuf>,
    /// A root file system to add.
    ///
    /// Incompatible with `zip`.
    #[arg(long = "root-fs")]
    root_fs: Option<PathBuf>,

    #[arg(long = "add-section")]
    extra_section: Vec<ExtraSection>,

    /// A boot process to setup processing, within wasi.
    ///
    /// This is a shorthand for the appropriate section or file addition, depending on the target.
    /// You can also enjoy this option being more stable if the boot details of any of these
    /// targets changes.
    #[arg(long = "stage3")]
    stage3: Option<PathBuf>,

    /// A customized section name to use for the final zip section.
    ///
    /// The section is named `wah_polyglot_stage2_data` by default.
    #[arg(long = "trailing-zip-section")]
    zip_section_name: Option<String>,

    /// How to wrap the output Web Assembly module.
    ///
    /// This determines the 'stage 0' entry point into setting up the web assembly. There are two
    /// options:
    ///
    /// * `wasm`, which enters execution from an initial section that looks like valid HTML. This
    ///   target is NOT compatible with serving from a file in Chromium, as it requires access to
    ///   the file's own file-URI via `fetch`.
    /// * `html`, which encodes the resulting module as a blob and loads it. This target is
    ///   generally compatible with web browsers but obviously the output file is no longer a
    ///   WebAssembly module itself.
    #[arg(long, short = 't', alias = "target", default_value = "wasm")]
    target: Target,

    // Experimental section.
    /// Experimental. Hot-reload when the WASM file changes.
    ///
    /// Details and use-case are not entirely fixed. Should we 'reboot' into the new stage0,
    /// stage1, or stage2? At the moment it calls the _old_ stage2 with the _new_ WASM data. This
    /// works with Yew Apps, for example.
    ///
    /// Must set the environment variable `WAH_POLYGLOT_EXPERIMENTAL` to use.
    #[arg(long, alias = "dev")]
    edit: bool,
}

#[derive(Clone)]
enum Target {
    WasmPlusHtml,
    Html,
    HtmlPlusTar,
}

impl core::str::FromStr for Target {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "wasm" => Ok(Self::WasmPlusHtml),
            "wasm+html" => Ok(Self::WasmPlusHtml),
            "html+tar" => Ok(Self::HtmlPlusTar),
            "html" => Ok(Self::Html),
            _ => Err(format!("Unknown target selection {s}")),
        }
    }
}

impl core::str::FromStr for ExtraSection {
    type Err = String;

    fn from_str(val: &str) -> Result<Self, Self::Err> {
        let Some((key, suffix)) = val.split_once(",") else {
            return Err("expected `section_name,file_name`".into());
        };

        Ok(ExtraSection {
            name: key.into(),
            from_file: suffix.into(),
        })
    }
}
