use std::io::Read;
use zip::ZipArchive;

const STAGE3_JS: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/boot.mjs"));

pub fn main() -> Result<(), zip::result::ZipError> {
    let mut stdin = std::io::stdin();
    let mut data = vec![];
    stdin.read_to_end(&mut data)?;

    let wasm_binary = std::fs::read("proc/self/exe")?;

    let data = std::io::Cursor::new(data);

    // Hm, maybe we should be more strict. This is only optional if we intended it to be.
    if let Ok(mut archive) = ZipArchive::new(data) {
        archive.extract("/")?;
    }

    std::fs::write("boot/index.mjs", STAGE3_JS)?;

    Ok(())
}

enum ProcResult {
    Ok,
    Err(zip::result::ZipError),
}

impl From<Result<(), zip::result::ZipError>> for ProcResult {
    fn from(res: Result<(), zip::result::ZipError>) -> Self {
        match res {
            Ok(()) => ProcResult::Ok,
            Err(err) => ProcResult::Err(err),
        }
    }
}

impl std::process::Termination for ProcResult {
    fn report(self) -> std::process::ExitCode {
        let ProcResult::Err(err) = self else {
            return std::process::ExitCode::SUCCESS;
        };

        use std::io::Write;
        let stderr = std::io::stderr();
        match write!(stderr.lock(), "{}", err) {
            Ok(_) => std::process::ExitCode::FAILURE,
            Err(_) => std::process::ExitCode::from(127),
        }
    }
}
