use std::env;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmd {
    pub program: String,
    pub args: Vec<String>,
}

impl Cmd {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
        }
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn display(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .map(shell_quote)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn shell_quote(value: &str) -> String {
    let safe = !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:=@%+,".contains(c));
    if safe {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmdOutput {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CmdOutput {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    pub fn summary(&self) -> String {
        let stderr = self.stderr.trim();
        if !stderr.is_empty() {
            stderr.to_string()
        } else {
            self.stdout.trim().to_string()
        }
    }
}

pub trait Runner {
    fn run(&self, cmd: &Cmd) -> io::Result<CmdOutput>;
    fn exists(&self, program: &str) -> bool;

    fn read_to_string(&self, path: &str) -> io::Result<String> {
        std::fs::read_to_string(path)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemRunner;

impl Runner for SystemRunner {
    fn run(&self, cmd: &Cmd) -> io::Result<CmdOutput> {
        let output = Command::new(&cmd.program)
            .env("LC_ALL", "C")
            .args(&cmd.args)
            .output()?;
        Ok(CmdOutput {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    fn exists(&self, program: &str) -> bool {
        if program.contains('/') || program.contains('\\') {
            return Path::new(program).is_file();
        }
        let Some(path) = env::var_os("PATH") else {
            return false;
        };
        let extensions: Vec<OsString> = if cfg!(windows) {
            let mut values = vec![OsString::new()];
            if let Some(pathext) = env::var_os("PATHEXT") {
                values.extend(
                    pathext
                        .to_string_lossy()
                        .split(';')
                        .filter(|extension| !extension.is_empty())
                        .map(OsString::from),
                );
            }
            values
        } else {
            vec![OsString::new()]
        };
        env::split_paths(&path).any(|directory| {
            extensions.iter().any(|extension| {
                let mut file_name = OsString::from(program);
                file_name.push(extension);
                directory.join(&file_name).is_file()
            })
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivilegeTools {
    pub is_root: bool,
    pub has_pkexec: bool,
    pub has_sudo: bool,
}

pub fn detect_privilege(runner: &dyn Runner) -> PrivilegeTools {
    let is_root = runner
        .run(&Cmd::new("id").arg("-u"))
        .ok()
        .map(|output| output.stdout.trim() == "0")
        .unwrap_or(false);
    PrivilegeTools {
        is_root,
        has_pkexec: runner.exists("pkexec"),
        has_sudo: runner.exists("sudo"),
    }
}

pub fn wrap_privileged(
    cmd: Cmd,
    tools: PrivilegeTools,
    prefer_pkexec: bool,
) -> Result<Cmd, String> {
    if tools.is_root {
        return Ok(cmd);
    }
    let wrapper = if prefer_pkexec && tools.has_pkexec {
        "pkexec"
    } else if !prefer_pkexec && tools.has_sudo {
        "sudo"
    } else if tools.has_pkexec {
        "pkexec"
    } else if tools.has_sudo {
        "sudo"
    } else {
        return Err("برای انجام عملیات مدیریتی به pkexec (polkit) یا sudo نیاز است.".to_string());
    };
    let mut args = Vec::with_capacity(cmd.args.len() + 1);
    args.push(cmd.program);
    args.extend(cmd.args);
    Ok(Cmd::new(wrapper).args(args))
}

pub fn run_privileged(
    runner: &dyn Runner,
    cmd: Cmd,
    prefer_pkexec: bool,
) -> Result<CmdOutput, String> {
    let tools = detect_privilege(runner);
    let wrapped = wrap_privileged(cmd, tools, prefer_pkexec)?;
    runner
        .run(&wrapped)
        .map_err(|error| format!("اجرای «{}» ناموفق بود: {error}", wrapped.display()))
}

pub fn describe_failure(cmd: &Cmd, output: &CmdOutput) -> String {
    let detail = if !output.stderr.trim().is_empty() {
        output.stderr.trim()
    } else {
        output.stdout.trim()
    };
    if detail.is_empty() {
        format!(
            "فرمان «{}» با کد {:?} شکست خورد.",
            cmd.display(),
            output.code
        )
    } else {
        format!("فرمان «{}» شکست خورد: {detail}", cmd.display())
    }
}
