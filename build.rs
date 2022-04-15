/// Vendored autocfg@1.1.0 to get access to raw probe fn to probe with features
/// cuviper/autocfg#24, cuviper/autocfg#28, cuviper/autocfg#35
mod autocfg {
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::io::{stderr, Write};
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Writes a config flag for rustc on standard out.
    ///
    /// This looks like: `cargo:rustc-cfg=CFG`
    ///
    /// Cargo will use this in arguments to rustc, like `--cfg CFG`.
    pub fn emit(cfg: &str) {
        println!("cargo:rustc-cfg={}", cfg);
    }

    /// Writes a line telling Cargo to rerun the build script if `path` changes.
    ///
    /// This looks like: `cargo:rerun-if-changed=PATH`
    ///
    /// This requires at least cargo 0.7.0, corresponding to rustc 1.6.0.  Earlier
    /// versions of cargo will simply ignore the directive.
    pub fn rerun_path(path: &str) {
        println!("cargo:rerun-if-changed={}", path);
    }

    /// Helper to detect compiler features for `cfg` output in build scripts.
    #[derive(Clone, Debug)]
    pub struct AutoCfg {
        out_dir: PathBuf,
        rustc: PathBuf,
        target: Option<OsString>,
        no_std: bool,
        rustflags: Vec<String>,
    }

    /// Create a new `AutoCfg` instance.
    ///
    /// # Panics
    ///
    /// Panics if `AutoCfg::new()` returns an error.
    pub fn new() -> AutoCfg {
        AutoCfg::new().unwrap()
    }

    impl AutoCfg {
        /// Create a new `AutoCfg` instance.
        ///
        /// # Common errors
        ///
        /// - `rustc` can't be executed, from `RUSTC` or in the `PATH`.
        /// - The version output from `rustc` can't be parsed.
        /// - `OUT_DIR` is not set in the environment, or is not a writable directory.
        ///
        pub fn new() -> Result<Self, std::io::Error> {
            match env::var_os("OUT_DIR") {
                Some(d) => Self::with_dir(d),
                None => Err(std::io::ErrorKind::NotFound.into()),
            }
        }

        /// Create a new `AutoCfg` instance with the specified output directory.
        ///
        /// # Common errors
        ///
        /// - `rustc` can't be executed, from `RUSTC` or in the `PATH`.
        /// - The version output from `rustc` can't be parsed.
        /// - `dir` is not a writable directory.
        ///
        pub fn with_dir<T: Into<PathBuf>>(dir: T) -> Result<Self, std::io::Error> {
            let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
            let rustc: PathBuf = rustc.into();

            let target = env::var_os("TARGET");

            // Sanity check the output directory
            let dir = dir.into();
            let meta = fs::metadata(&dir)?;
            if !meta.is_dir() || meta.permissions().readonly() {
                return Err(std::io::ErrorKind::PermissionDenied.into());
            }

            let mut ac = AutoCfg {
                rustflags: rustflags(),
                out_dir: dir,
                rustc,
                target,
                no_std: false,
            };

            // Sanity check with and without `std`.
            if !ac.probe("").unwrap_or(false) {
                ac.no_std = true;
                if !ac.probe("").unwrap_or(false) {
                    // Neither worked, so assume nothing...
                    ac.no_std = false;
                    let warning = b"warning: autocfg could not probe for `std`\n";
                    stderr().write_all(warning).ok();
                }
            }
            Ok(ac)
        }

        pub fn probe<T: AsRef<[u8]>>(&self, code: T) -> Result<bool, std::io::Error> {
            static ID: AtomicUsize = AtomicUsize::new(0);

            let id = ID.fetch_add(1, Ordering::Relaxed);
            let mut command = Command::new(&self.rustc);
            command
                .arg("--crate-name")
                .arg(format!("probe{}", id))
                .arg("--crate-type=lib")
                .arg("--out-dir")
                .arg(&self.out_dir)
                .arg("--emit=llvm-ir");

            if let Some(target) = self.target.as_ref() {
                command.arg("--target").arg(target);
            }

            command.args(&self.rustflags);

            command.arg("-").stdin(Stdio::piped());
            let mut child = command.spawn()?;
            let mut stdin = child.stdin.take().expect("rustc stdin");

            if self.no_std {
                stdin.write_all(b"#![no_std]\n")?;
            }
            stdin.write_all(code.as_ref())?;
            drop(stdin);

            let status = child.wait()?;
            Ok(status.success())
        }
    }

    fn rustflags() -> Vec<String> {
        // Starting with rust-lang/cargo#9601, shipped in Rust 1.55, Cargo always sets
        // CARGO_ENCODED_RUSTFLAGS for any host/target build script invocation. This
        // includes any source of flags, whether from the environment, toml config, or
        // whatever may come in the future. The value is either an empty string, or a
        // list of arguments separated by the ASCII unit separator (US), 0x1f.
        if let Ok(a) = env::var("CARGO_ENCODED_RUSTFLAGS") {
            return if a.is_empty() {
                Vec::new()
            } else {
                a.split('\x1f').map(str::to_string).collect()
            };
        }
        panic!("expected $env:CARGO_ENCODED_RUSTFLAGS");
    }
}

fn main() {
    autocfg::rerun_path("build.rs");

    let autocfg = autocfg::new();
    let has_simple_decl_macro = autocfg
        .probe(
            r##"
                #![feature(decl_macro, rustc_attrs)]
                #[rustc_macro_transparency = "semitransparent"]
                pub macro m {
                    () => {},
                    () => {},
                }
            "##,
        )
        .unwrap_or_default();
    if has_simple_decl_macro {
        autocfg::emit("has_simple_decl_macro");
    }
}
