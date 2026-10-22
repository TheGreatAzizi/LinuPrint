#![allow(dead_code)]

use std::collections::HashMap;
use std::io;

use linuprint::backend::shell::{Cmd, CmdOutput, Runner};

pub struct FakeRunner {
    pub present: Vec<String>,
    pub responses: HashMap<String, CmdOutput>,
    pub files: HashMap<String, String>,
    pub default: CmdOutput,
}

impl FakeRunner {
    pub fn new() -> Self {
        Self {
            present: Vec::new(),
            responses: HashMap::new(),
            files: HashMap::new(),
            default: success(""),
        }
    }

    pub fn with_tools(tools: &[&str]) -> Self {
        let mut runner = Self::new();
        runner.present = tools.iter().map(|tool| (*tool).to_string()).collect();
        runner
    }

    pub fn on(&mut self, command: &str, output: CmdOutput) -> &mut Self {
        self.responses.insert(command.to_string(), output);
        self
    }

    pub fn with_file(&mut self, path: &str, contents: &str) -> &mut Self {
        self.files.insert(path.to_string(), contents.to_string());
        self
    }

    fn key(cmd: &Cmd) -> String {
        if cmd.args.is_empty() {
            cmd.program.clone()
        } else {
            format!("{} {}", cmd.program, cmd.args.join(" "))
        }
    }
}

impl Runner for FakeRunner {
    fn run(&self, cmd: &Cmd) -> io::Result<CmdOutput> {
        let key = Self::key(cmd);
        Ok(self
            .responses
            .get(&key)
            .cloned()
            .unwrap_or_else(|| self.default.clone()))
    }

    fn exists(&self, program: &str) -> bool {
        self.present.iter().any(|present| present == program)
    }

    fn read_to_string(&self, path: &str) -> io::Result<String> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("{path} not found")))
    }
}

pub fn success(stdout: &str) -> CmdOutput {
    CmdOutput {
        code: Some(0),
        stdout: stdout.to_string(),
        stderr: String::new(),
    }
}

pub fn failure(code: i32, stderr: &str) -> CmdOutput {
    CmdOutput {
        code: Some(code),
        stdout: String::new(),
        stderr: stderr.to_string(),
    }
}
