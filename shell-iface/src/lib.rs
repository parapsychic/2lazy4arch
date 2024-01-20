use std::process::{Command, ExitStatus};

use anyhow::Result;

// So that I don't accidently run it on my working machine and blast it to pieces
pub enum RunMode {
    Testing,
    Release,
}

pub struct ShellCmd {
    command: String,
    status: ExitStatus,
}

pub struct Shell {
    build_mode: RunMode,
    last_run_cmd: Option<ShellCmd>,
}

impl Shell {
    pub fn new() -> Self {
        Self {
            build_mode: RunMode::Testing,
            last_run_cmd: None
        }
    }

    pub fn run(&self, cmd: &str, args: &str) -> Result<ExitStatus> {
        if let RunMode::Testing = &self.build_mode {
            println!(
                "Running Shell in Test Mode: Command: {} Args: {}",
                cmd, args
            );
            return Ok(ExitStatus::default());
        }

        let output = Command::new(cmd).arg(args).output()?;
        Ok(output.status)
    }
}
