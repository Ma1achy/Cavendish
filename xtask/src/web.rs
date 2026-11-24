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
    // The forward stage's Web Worker: the same module, started by this script.
    std::fs::copy(root.join("crates/cav-app/web/worker.js"), dist.join("worker.js"))
        .map_err(|e| format!("crates/cav-app/web/worker.js: {e}"))?;
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
    // In the `web` profile: the optimisation, LTO and overflow checks the page is built with.
    let mut wasm = cargo(
        root,
        &[
            "test",
            "--profile",
            "web",
            "--target",
            TARGET,
            "-p",
            "cav-engine",
            "--test",
            "web_equivalence",
            "--",
            "--nocapture",
        ],
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

/// A static server for `target/web/dist` on `127.0.0.1`, in this process: the files under that
/// folder (`index.html`, the glue, the module with `application/wasm`), nothing else. It stops
/// when the returned [`Server`] is dropped. For the demo and `cargo xtask web serve`; not for the
/// internet.
///
/// What keeps it to its folder and keeps it answering:
/// - a request's path is a plain relative path of letters, digits, `.`, `-` and `_` between
///   slashes ([`resolve`]): absolute paths, drive letters, backslashes, `..` and `.`, empty parts
///   and every encoded form (`%`) are refused, and the file found must still be under the
///   folder once both are made canonical (a link out of it is refused too);
/// - each connection has a thread of its own, at most [`MAX_CONNECTIONS`] at once (one more is
///   closed at once), and a read or a write that stalls for [`IO_TIMEOUT`] ends its connection:
///   an idle socket (a browser's speculative preconnect) holds one thread for that long, never
///   the server.
pub struct Server {
    /// The port it listens on.
    pub port: u16,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// The most connections served at once.
pub const MAX_CONNECTIONS: usize = 16;
/// How long a connection may wait for the client's next bytes, or the client for its answer.
pub const IO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
/// The longest request head read: a line longer than this ends the connection.
const MAX_HEAD: u64 = 16 << 10;

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
    serve_for(dir, port, IO_TIMEOUT)
}

/// [`serve`] with a stated timeout for a connection's reads and writes.
fn serve_for(dir: &Path, port: u16, timeout: std::time::Duration) -> Result<Server, String> {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let dir = dir.canonicalize().map_err(|e| format!("{}: {e}", dir.display()))?;
    let listener = std::net::TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("serving: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let stopped = std::sync::Arc::clone(&stop);
    let dir = std::sync::Arc::new(dir);
    let open = std::sync::Arc::new(AtomicUsize::new(0));
    let thread = std::thread::spawn(move || {
        while !stopped.load(Ordering::Relaxed) {
            let Ok((stream, _)) = listener.accept() else {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            };
            if open.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                open.fetch_sub(1, Ordering::SeqCst);
                drop(stream);
                continue;
            }
            let (dir, done) = (std::sync::Arc::clone(&dir), std::sync::Arc::clone(&open));
            let spawned = std::thread::Builder::new().name("xtask-serve".into()).spawn(move || {
                answer(&stream, &dir, timeout);
                done.fetch_sub(1, Ordering::SeqCst);
            });
            if spawned.is_err() {
                open.fetch_sub(1, Ordering::SeqCst);
            }
        }
    });
    Ok(Server { port, stop, thread: Some(thread) })
}

/// Answers one connection: one request, then closed.
fn answer(stream: &std::net::TcpStream, dir: &Path, timeout: std::time::Duration) {
    use std::io::{BufRead, BufReader, Read, Write};
    if stream.set_nonblocking(false).is_err()
        || stream.set_read_timeout(Some(timeout)).is_err()
        || stream.set_write_timeout(Some(timeout)).is_err()
    {
        return;
    }
    let mut reader = BufReader::new(stream.take(MAX_HEAD));
    let mut request = String::new();
    if reader.read_line(&mut request).is_err() {
        return;
    }
    // The rest of the head is read and ignored.
    let mut line = String::new();
    while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
        line.clear();
    }
    let target = request.split(' ').nth(1).unwrap_or("");
    let mut stream = stream;
    match resolve(dir, target).and_then(|(file, name)| Some((std::fs::read(file).ok()?, name))) {
        Some((bytes, name)) => {
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
                 Connection: close\r\n\r\n",
                content_type(&name),
                bytes.len()
            );
            let _ = stream.write_all(&bytes);
        }
        None => {
            let _ =
                write!(stream, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        }
    }
}

/// The file under `dir` (canonical) a request's target names, with its path as asked; `None`
/// for anything that is not a plain relative path to a file under `dir`. `/` is `index.html`;
/// a query (`?…`) is ignored.
fn resolve(dir: &Path, target: &str) -> Option<(std::path::PathBuf, String)> {
    let path = target.split('?').next()?;
    let path = path.strip_prefix('/')?;
    let path = if path.is_empty() { "index.html" } else { path };
    let plain = |part: &str| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && part.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
    };
    if !path.split('/').all(plain) {
        return None;
    }
    let file = dir.join(path).canonicalize().ok()?;
    (file.starts_with(dir) && file.is_file()).then(|| (file, path.to_owned()))
}

/// The media type a file of the page is served as.
fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript",
        Some("wasm") => "application/wasm",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}

/// `cargo xtask web serve [--exit-after <seconds>]`: serves the built page on port 8000 until
/// the process is stopped, or for `exit_after` and then stops by itself. Scripts and agents
/// always give a time: on Windows, stopping `cargo` does not stop the `xtask.exe` it started.
pub fn serve_forever(root: &Path, exit_after: Option<std::time::Duration>) -> Result<(), String> {
    let dist = root.join(DIST);
    if !dist.join("index.html").is_file() {
        return Err(format!("{DIST} has no page: `cargo xtask web build` first"));
    }
    let server = serve(&dist, 8000)?;
    match exit_after {
        Some(time) => {
            eprintln!(
                "xtask: serving {DIST} at http://127.0.0.1:{}/ for {} s (Ctrl-C stops it sooner)",
                server.port,
                time.as_secs()
            );
            std::thread::sleep(time);
            drop(server);
            eprintln!("xtask: stopped serving after {} s", time.as_secs());
            Ok(())
        }
        None => {
            eprintln!("xtask: serving {DIST} at http://127.0.0.1:{}/ (Ctrl-C stops it)", server.port);
            loop {
                std::thread::park();
            }
        }
    }
}

/// The seconds of `--exit-after <seconds>`: a whole number from 1 to a day.
pub fn exit_after(seconds: &str) -> Result<std::time::Duration, String> {
    match seconds.parse::<u64>() {
        Ok(s) if (1..=86_400).contains(&s) => Ok(std::time::Duration::from_secs(s)),
        _ => Err(format!("--exit-after wants whole seconds from 1 to 86400, not `{seconds}`")),
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
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub").join("inner.js"), "x").unwrap();
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
        assert!(get("/sub/inner.js?v=1").starts_with("HTTP/1.1 200"), "a file in a folder, a query ignored");
        // Control arms: a missing file and every way out of the folder are not served. The
        // file outside is real and readable, so a 404 is the server refusing, not a miss.
        assert!(get("/nothing.js").starts_with("HTTP/1.1 404"));
        let outside = dir.parent().unwrap().join(format!("cav-xtask-outside-{}.txt", std::process::id()));
        std::fs::write(&outside, "secret").unwrap();
        let name = outside.file_name().unwrap().to_string_lossy().into_owned();
        let absolute = outside.to_string_lossy().replace('\\', "/");
        let escapes = [
            format!("/../{name}"),
            format!("/sub/../../{name}"),
            format!("/{absolute}"), // `/C:/…/x`: a drive letter
            format!("//{}", absolute.trim_start_matches('/')), // `//C:/…` or `//tmp/…`
            format!("/..\\{name}"), // a backslash
            format!("/sub\\..\\..\\{name}"),
            format!("/%2e%2e/{name}"),   // encoded dots
            format!("/..%2f{name}"),     // an encoded slash
            format!("/%2E%2E%5C{name}"), // encoded backslash
            "/./index.html".to_owned(),  // a `.` part
            "/sub//inner.js".to_owned(), // an empty part
        ];
        for escape in &escapes {
            let said = get(escape);
            assert!(said.starts_with("HTTP/1.1 404"), "{escape} was served: {said}");
            assert!(!said.contains("secret"), "{escape}");
        }
        // The resolver alone, with the checks the server makes: a plain path inside resolves.
        let canonical = dir.canonicalize().unwrap();
        assert!(resolve(&canonical, "/sub/inner.js").is_some());
        assert!(resolve(&canonical, &format!("/{absolute}")).is_none());
        std::fs::remove_file(&outside).unwrap();
        drop(server);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_idle_connection_does_not_stop_the_server() {
        use std::io::{Read, Write};
        let dir = std::env::temp_dir().join(format!("cav-xtask-idle-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("index.html"), "<p>hi</p>").unwrap();
        // A short timeout, so that the control below can wait it out.
        let timeout = std::time::Duration::from_millis(400);
        let server = serve_for(&dir, 0, timeout).unwrap();
        let get = || {
            let mut stream = std::net::TcpStream::connect(("127.0.0.1", server.port)).unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();
            write!(stream, "GET / HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
            let mut said = String::new();
            let _ = stream.read_to_string(&mut said);
            said
        };
        // Speculative preconnects: sockets opened and never written to, more than one.
        let idle: Vec<_> =
            (0..3).map(|_| std::net::TcpStream::connect(("127.0.0.1", server.port)).unwrap()).collect();
        std::thread::sleep(std::time::Duration::from_millis(100));
        let started = std::time::Instant::now();
        let page = get();
        assert!(page.starts_with("HTTP/1.1 200"), "a request behind idle sockets: {page:?}");
        assert!(started.elapsed() < timeout, "it waited for an idle socket's timeout");
        // Control: the cap is real. With every slot held by an idle socket a request is closed
        // unanswered; once the idle ones time out (still open on this side) it answers again.
        let more: Vec<_> = (0..MAX_CONNECTIONS)
            .map(|_| std::net::TcpStream::connect(("127.0.0.1", server.port)).unwrap())
            .collect();
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert_eq!(get(), "", "a connection past the cap is closed at once");
        std::thread::sleep(timeout * 2);
        assert!(get().starts_with("HTTP/1.1 200"), "the idle sockets timed out");
        drop((idle, more));
        drop(server);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Where a `cfg` on the target's architecture may appear (`docs/extending/the-web-build.md`):
/// only on a module's declaration, choosing a platform seam's module, so that everything else
/// is one path for both. Answers each place in `text` (one file's) that breaks the rule, as
/// `line: text`.
#[cfg(test)]
fn stray_cfgs(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut stray = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let code = line.split("//").next().unwrap_or_default();
        if !code.contains("target_arch") {
            continue;
        }
        let next = lines.get(index + 1).map_or("", |next| next.trim_start());
        let declares = code.trim_start().starts_with("#[cfg(")
            && (next.starts_with("#[path") || next.starts_with("mod ") || next.starts_with("pub mod "));
        if !declares {
            stray.push(format!("{}: {}", index + 1, line.trim()));
        }
    }
    stray
}

#[cfg(test)]
mod seams {
    use super::stray_cfgs;

    /// Every `.rs` file under `dir`.
    fn sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    #[test]
    fn only_the_seams_name_the_target() {
        // Every crate's own code (`src/`): a `cfg(target_arch …)` only where a seam chooses its
        // module. (Tests may name the target: the web equivalence is a wasm test.)
        let mut files = Vec::new();
        for krate in std::fs::read_dir(crate::root().join("crates")).unwrap() {
            let src = krate.unwrap().path().join("src");
            if src.is_dir() {
                sources(&src, &mut files);
            }
        }
        assert!(files.len() > 50, "the crates' sources were not found");
        let stray: Vec<String> = files
            .iter()
            .flat_map(|file| {
                let text = std::fs::read_to_string(file).unwrap();
                stray_cfgs(&text).into_iter().map(move |at| format!("{}:{at}", file.display()))
            })
            .collect();
        assert!(
            stray.is_empty(),
            "cfg(target_arch) outside a seam's module declaration:\n{}",
            stray.join("\n")
        );
        // Control arms: a cfg on a statement, on a function, a cfg_attr and a cfg! are found; a
        // seam's declaration (with or without #[path]) and a comment are not.
        let seam = "#[cfg(not(target_arch = \"wasm32\"))]\n#[path = \"x/native.rs\"]\nmod platform;\n\
                    #[cfg(target_arch = \"wasm32\")]\npub mod web;\n// target_arch is chosen above\n";
        assert!(stray_cfgs(seam).is_empty());
        for wrong in [
            "fn f() {\n    #[cfg(target_arch = \"wasm32\")]\n    crate::web::after_frame();\n}\n",
            "#[cfg(target_arch = \"wasm32\")]\nfn only_on_the_web() {}\n",
            "#![cfg_attr(target_arch = \"wasm32\", allow(clippy::x))]\n",
            "let web = cfg!(target_arch = \"wasm32\");\n",
        ] {
            assert_eq!(stray_cfgs(wrong).len(), 1, "{wrong}");
        }
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
