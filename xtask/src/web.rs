//! `cargo xtask web`: the application as a web page (`docs/slices/S17.md`).
//!
//! `cargo xtask web build` builds `target/web/dist/` (what GitHub Pages serves): the `cav-web`
//! binary for `wasm32-unknown-unknown` in the `web` profile, wasm-bindgen's glue, wasm-opt, and
//! the page with its revision stamped. The Pages workflow runs exactly this.
//!
//! `cargo xtask web` is the tier: the web code's lints (native clippy does not compile it), the
//! build, and the engine's numbers in wasm held to the native ones (`web_equivalence`).
//!
//! The tools are pinned: wasm-bindgen must be the version `Cargo.lock` has (the glue and the crate
//! must agree), wasm-opt is binaryen's `version_133`. Neither is installed by this command.

use std::path::Path;
use std::process::Command;

use crate::{cargo, step};

/// Where the page is written.
pub const DIST: &str = "target/web/dist";
/// The binaryen release wasm-opt must come from.
const BINARYEN: &str = "version_133";
/// The wasm features rustc 1.100 turns on for this target by default, which every browser with
/// WebGPU has; wasm-opt is told so, and keeps to them.
const FEATURES: [&str; 6] = [
    "--enable-bulk-memory",
    "--enable-sign-ext",
    "--enable-reference-types",
    "--enable-multivalue",
    "--enable-mutable-globals",
    "--enable-nontrapping-float-to-int",
];
const TARGET: &str = "wasm32-unknown-unknown";

/// The tier: lints, the build, the equivalence of the numbers.
pub fn tier(root: &Path) -> Result<(), String> {
    step(
        "clippy (wasm32)",
        cargo(
            root,
            &[
                "clippy", "--target", TARGET, "-p", "cav-app", "--lib", "--bin", "cav-web", "--quiet", "--",
                "-D", "warnings",
            ],
        ),
    )?;
    build(root)?;
    equivalence(root)
}

/// Builds `target/web/dist`.
pub fn build(root: &Path) -> Result<(), String> {
    let bindgen = wasm_bindgen_version(root)?;
    check_tool(
        "wasm-bindgen",
        &["--version"],
        &format!("wasm-bindgen {bindgen}"),
        &format!(
            "cargo install wasm-bindgen-cli --version {bindgen} --locked (or the release's prebuilt binary)"
        ),
    )?;
    check_tool(
        "wasm-opt",
        &["--version"],
        &format!("({BINARYEN})"),
        &format!("binaryen {BINARYEN} from https://github.com/WebAssembly/binaryen/releases/tag/{BINARYEN}"),
    )?;
    step(
        "build (wasm32, profile web)",
        cargo(root, &["build", "--profile", "web", "--target", TARGET, "-p", "cav-app", "--bin", "cav-web"]),
    )?;
    let dist = root.join(DIST);
    if dist.exists() {
        std::fs::remove_dir_all(&dist).map_err(|e| format!("{}: not emptied: {e}", dist.display()))?;
    }
    let built = root.join("target").join(TARGET).join("web").join("cav-web.wasm");
    let mut bindgen = Command::new("wasm-bindgen");
    bindgen.current_dir(root).args(["--target", "web", "--no-typescript", "--out-dir", DIST, "--out-name"]);
    bindgen.arg("cavendish").arg(&built);
    step("wasm-bindgen", bindgen)?;
    let module = dist.join("cavendish_bg.wasm");
    let before = size(&module)?;
    let mut opt = Command::new("wasm-opt");
    opt.args(["-O2", "-o"]).arg(&module).arg(&module).args(FEATURES);
    step("wasm-opt -O2", opt)?;
    let page = std::fs::read_to_string(root.join("crates/cav-app/web/index.html"))
        .map_err(|e| format!("crates/cav-app/web/index.html: {e}"))?;
    let page = page.replace("{{revision}}", &revision(root));
    std::fs::write(dist.join("index.html"), page).map_err(|e| format!("{DIST}/index.html: {e}"))?;
    eprintln!(
        "xtask: web: {DIST}: cav-web.wasm {} bytes from cargo, {before} after wasm-bindgen, {} after wasm-opt; \
         cavendish.js {} bytes",
        size(&built)?,
        size(&module)?,
        size(&dist.join("cavendish.js"))?
    );
    Ok(())
}

/// The engine's numbers in wasm, under wasm-bindgen's runner in Node.js, against the native ones.
fn equivalence(root: &Path) -> Result<(), String> {
    let numbers = root.join("target/web/equivalence-native.txt");
    let numbers = numbers.to_string_lossy().replace('\\', "/");
    let mut native =
        cargo(root, &["test", "-p", "cav-engine", "--test", "web_equivalence", "--", "--ignored"]);
    native.env("CAV_WEB_EQUIVALENCE", &numbers);
    step("equivalence: native numbers", native)?;
    check_tool("wasm-bindgen-test-runner", &["--version"], "", "it comes with wasm-bindgen-cli")?;
    let mut wasm = cargo(
        root,
        &["test", "--target", TARGET, "-p", "cav-engine", "--test", "web_equivalence", "--", "--nocapture"],
    );
    wasm.env("CAV_WEB_EQUIVALENCE", &numbers)
        .env("CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER", "wasm-bindgen-test-runner");
    step("equivalence: wasm against native (Node.js)", wasm)
}

/// The wasm-bindgen version `Cargo.lock` holds.
fn wasm_bindgen_version(root: &Path) -> Result<String, String> {
    let lock = std::fs::read_to_string(root.join("Cargo.lock")).map_err(|e| format!("Cargo.lock: {e}"))?;
    lock_version(&lock, "wasm-bindgen").ok_or_else(|| "Cargo.lock has no wasm-bindgen".into())
}

/// The version of package `name` in a lock file's text.
fn lock_version(lock: &str, name: &str) -> Option<String> {
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line == format!("name = \"{name}\"") {
            let version = lines.next()?.strip_prefix("version = \"")?.strip_suffix('"')?;
            return Some(version.to_owned());
        }
    }
    None
}

/// Runs `tool` with `args` and fails, saying how to get it, unless its output contains `wanted`.
fn check_tool(tool: &str, args: &[&str], wanted: &str, install: &str) -> Result<(), String> {
    let output =
        Command::new(tool).args(args).output().map_err(|e| format!("{tool}: not found ({e}); {install}"))?;
    let said = String::from_utf8_lossy(&output.stdout);
    if said.contains(wanted) {
        Ok(())
    } else {
        Err(format!("{tool} says `{}`; this build needs `{wanted}`: {install}", said.trim()))
    }
}

fn size(path: &Path) -> Result<u64, String> {
    std::fs::metadata(path).map(|m| m.len()).map_err(|e| format!("{}: {e}", path.display()))
}

/// The commit the page is built from, as git names it (`GITHUB_SHA` in the workflow is the same).
fn revision(root: &Path) -> String {
    Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map_or_else(
            || "unknown".to_owned(),
            |output| String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        )
}

/// A static server for `target/web/dist` on `127.0.0.1`, in this process: what the page needs
/// (`index.html`, the glue, the module with `application/wasm`), nothing else. It stops when the
/// returned [`Server`] is dropped. For the demo and `cargo xtask web serve`; not for the internet.
pub struct Server {
    /// The port it listens on.
    pub port: u16,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Serves `dir` on `127.0.0.1:port` (0: any free port).
pub fn serve(dir: &Path, port: u16) -> Result<Server, String> {
    use std::io::{BufRead, BufReader, Write};
    use std::sync::atomic::{AtomicBool, Ordering};
    let listener = std::net::TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("serving: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let (dir, stopped) = (dir.to_path_buf(), std::sync::Arc::clone(&stop));
    let thread = std::thread::spawn(move || {
        while !stopped.load(Ordering::Relaxed) {
            let Ok((stream, _)) = listener.accept() else {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            };
            let _ = stream.set_nonblocking(false);
            let mut reader = BufReader::new(&stream);
            let mut request = String::new();
            if reader.read_line(&mut request).is_err() {
                continue;
            }
            // The rest of the head is read and ignored.
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
                line.clear();
            }
            let path = request.split(' ').nth(1).unwrap_or("/");
            let path = path.split(['?', '#']).next().unwrap_or("/").trim_start_matches('/');
            let path = if path.is_empty() { "index.html" } else { path };
            let file = (!path.contains("..")).then(|| dir.join(path));
            let mut stream = &stream;
            match file.and_then(|file| std::fs::read(&file).ok()) {
                Some(bytes) => {
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
                         Connection: close\r\n\r\n",
                        content_type(path),
                        bytes.len()
                    );
                    let _ = stream.write_all(&bytes);
                }
                None => {
                    let _ = write!(
                        stream,
                        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                }
            }
        }
    });
    Ok(Server { port, stop, thread: Some(thread) })
}

/// The media type a file of the page is served as.
fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript",
        Some("wasm") => "application/wasm",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
}

/// `cargo xtask web serve`: serves the built page on port 8000 until the process is stopped.
pub fn serve_forever(root: &Path) -> Result<(), String> {
    let dist = root.join(DIST);
    if !dist.join("index.html").is_file() {
        return Err(format!("{DIST} has no page: `cargo xtask web build` first"));
    }
    let server = serve(&dist, 8000)?;
    eprintln!("xtask: serving {DIST} at http://127.0.0.1:{}/ (Ctrl-C stops it)", server.port);
    loop {
        std::thread::park();
    }
}

/// A browser with WebGPU to take pictures with, headless: `CAV_CHROME` if set, else Chrome or Edge
/// where they are usually installed.
pub fn chrome() -> Result<std::path::PathBuf, String> {
    if let Ok(path) = std::env::var("CAV_CHROME") {
        return Ok(path.into());
    }
    let candidates = [
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        "/usr/bin/google-chrome",
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    ];
    candidates
        .iter()
        .map(std::path::PathBuf::from)
        .find(|path| path.is_file())
        .ok_or_else(|| "no Chrome or Edge found: set CAV_CHROME to one".into())
}

#[cfg(test)]
mod serving {
    use super::*;

    #[test]
    fn the_server_gives_the_module_its_type_and_nothing_outside() {
        use std::io::{Read, Write};
        let dir = std::env::temp_dir().join(format!("cav-xtask-serve-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("index.html"), "<p>hi</p>").unwrap();
        std::fs::write(dir.join("m.wasm"), [0, 97, 115, 109]).unwrap();
        let server = serve(&dir, 0).unwrap();
        let get = |path: &str| {
            let mut stream = std::net::TcpStream::connect(("127.0.0.1", server.port)).unwrap();
            write!(stream, "GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
            let mut said = String::new();
            stream.read_to_string(&mut said).unwrap();
            said
        };
        let page = get("/");
        assert!(
            page.starts_with("HTTP/1.1 200") && page.contains("text/html") && page.ends_with("<p>hi</p>")
        );
        assert!(get("/m.wasm").contains("Content-Type: application/wasm"));
        // Control arms: a missing file and a path out of the folder are not served.
        assert!(get("/nothing.js").starts_with("HTTP/1.1 404"));
        assert!(get("/../Cargo.toml").starts_with("HTTP/1.1 404"));
        drop(server);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lock_names_wasm_bindgens_version() {
        let lock = "[[package]]\nname = \"wasm-bindgen-futures\"\nversion = \"0.4.80\"\n\n\
                    [[package]]\nname = \"wasm-bindgen\"\nversion = \"0.2.130\"\n";
        assert_eq!(lock_version(lock, "wasm-bindgen").as_deref(), Some("0.2.130"));
        // Control: a name that is only a prefix of another is not found in it.
        assert_eq!(lock_version(lock, "wasm"), None);
        // The real lock has one, and the version the workflow and the tier pin.
        let real = std::fs::read_to_string(crate::root().join("Cargo.lock")).unwrap();
        assert!(lock_version(&real, "wasm-bindgen").is_some());
    }
}
