//! Runs external programs as argument lists (never through a shell), with a timeout.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
pub struct CmdOutput {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    /// The program could not be started.
    pub spawn_error: Option<String>,
    pub duration: Duration,
}

impl CmdOutput {
    pub fn ok(&self) -> bool {
        self.code == Some(0) && !self.timed_out && self.spawn_error.is_none()
    }
    /// stdout + stderr (fastboot prints to stderr).
    pub fn combined(&self) -> String {
        [self.stdout.trim_end(), self.stderr.trim_end()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join("\n")
    }
    pub fn failure(&self) -> String {
        if let Some(e) = &self.spawn_error {
            return e.clone();
        }
        if self.timed_out {
            return format!("El comando no respondió en {} s", self.duration.as_secs());
        }
        let tail = self.combined().lines().last().unwrap_or_default().to_string();
        format!("Código de salida {}{}", self.code.map_or("—".into(), |c| c.to_string()), if tail.is_empty() { String::new() } else { format!(": {tail}") })
    }
}

pub trait Runner: Send + Sync {
    fn run(&self, program: &Path, args: &[String], timeout: Duration) -> CmdOutput;
}

pub struct SystemRunner;

impl Runner for SystemRunner {
    fn run(&self, program: &Path, args: &[String], timeout: Duration) -> CmdOutput {
        let start = Instant::now();
        let mut command = Command::new(program);
        command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => {
                return CmdOutput {
                    spawn_error: Some(format!("No se pudo ejecutar {}: {e}", program.display())),
                    duration: start.elapsed(),
                    ..Default::default()
                }
            }
        };
        let mut out = child.stdout.take().expect("piped");
        let mut err = child.stderr.take().expect("piped");
        let out_reader = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = out.read_to_string(&mut s);
            s
        });
        let err_reader = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = err.read_to_string(&mut s);
            s
        });
        let mut timed_out = false;
        let code = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status.code(),
                Ok(None) if start.elapsed() >= timeout => {
                    timed_out = true;
                    let _ = child.kill();
                    break child.wait().ok().and_then(|s| s.code());
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(15)),
                Err(_) => break None,
            }
        };
        CmdOutput {
            code,
            stdout: out_reader.join().unwrap_or_default(),
            stderr: err_reader.join().unwrap_or_default(),
            timed_out,
            spawn_error: None,
            duration: start.elapsed(),
        }
    }
}

#[cfg(test)]
pub mod fake {
    //! Test double mapping argument lists to canned outputs.
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct FakeRunner {
        responses: Mutex<HashMap<String, CmdOutput>>,
        pub calls: Mutex<Vec<String>>,
    }

    impl FakeRunner {
        pub fn on(&self, args: &str, stdout: &str) -> &Self {
            self.respond(args, CmdOutput { code: Some(0), stdout: stdout.into(), ..Default::default() })
        }
        pub fn on_stderr(&self, args: &str, stderr: &str) -> &Self {
            self.respond(args, CmdOutput { code: Some(0), stderr: stderr.into(), ..Default::default() })
        }
        pub fn respond(&self, args: &str, output: CmdOutput) -> &Self {
            self.responses.lock().unwrap().insert(args.to_string(), output);
            self
        }
    }

    impl Runner for FakeRunner {
        fn run(&self, program: &Path, args: &[String], _timeout: Duration) -> CmdOutput {
            let tool = program.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let key = format!("{tool} {}", args.join(" "));
            self.calls.lock().unwrap().push(key.clone());
            self.responses.lock().unwrap().get(&key).cloned().unwrap_or(CmdOutput {
                code: Some(1),
                stderr: format!("sin respuesta simulada para: {key}"),
                ..Default::default()
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn runs_without_shell_and_times_out() {
        let r = SystemRunner;
        let out = r.run(Path::new("/bin/echo"), &["a; rm -rf /tmp/x".into()], Duration::from_secs(5));
        assert!(out.ok());
        assert_eq!(out.stdout.trim(), "a; rm -rf /tmp/x", "arguments are never interpreted by a shell");
        let slow = r.run(Path::new("/bin/sleep"), &["5".into()], Duration::from_millis(300));
        assert!(slow.timed_out && !slow.ok());
        let missing = r.run(Path::new("/no/such/adb"), &[], Duration::from_secs(1));
        assert!(missing.spawn_error.is_some());
    }
}
