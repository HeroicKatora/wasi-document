#[link(wasm_import_module = "wah_wasi")]
extern "C" {
    fn length() -> usize;
    fn get(ptr: usize);
    fn put(ptr: usize, len: usize);
}

const INST_SKIP: u32 = 1;
const INST_STRING: u32 = 2;
const INST_SET: u32 = 7;
const INST_UNZIP: u32 = 12;
const INST_SECTION: u32 = 13;

// Start of user-defined stack values.
const OPS: u32 = 14;
const ZIP_SECTION: &[u8] = b"wah_polyglot_stage2_data";
const ENV_DIR: &[u8] = b"dir";

#[no_mangle]
pub fn configure() {
    let len = unsafe { length() };

    let mut buffer = vec![0; len];
    unsafe { get(buffer.as_mut_ptr() as usize) };

    // Here, parse the configuration and setup WASI.
    let mut instructions = vec![0u8; 0];
    // string/2(_, 13)
    // unzip/1(_)
    // skip/1(13)
    #[rustfmt::skip]
    let mk_hello_world = &mut [
        INST_STRING, 2, 0, ZIP_SECTION.len() as u32,
        INST_SECTION, 1, OPS + 0,
        INST_UNZIP, 1, OPS + 1,
        INST_STRING, 2, 0, ENV_DIR.len() as u32,
        INST_SET, 3, 0, OPS + 3, OPS + 2,
        INST_SKIP, 1, ZIP_SECTION.len() as u32,
    ];

    let mut offset = bytemuck::bytes_of(mk_hello_world).len() as u32;
    mk_hello_world[2] = offset;
    offset += ZIP_SECTION.len() as u32;
    mk_hello_world[12] = offset;

    instructions.extend_from_slice(bytemuck::bytes_of(mk_hello_world));
    instructions.extend_from_slice(ZIP_SECTION);
    instructions.extend_from_slice(ENV_DIR);

    let pad = 3 - (instructions.len() + 3) & 0x3;
    if pad > 0 {
        instructions.extend_from_slice(&[0; 4][..pad]);
    }

    unsafe { put(instructions.as_ptr() as usize, instructions.len()) };
}
