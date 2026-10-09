use std::{
    fmt::Debug,
    io::Write,
    process::{Child, Command, ExitStatus, Output, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use logger::Logger;
pub mod logger;

static ECHO: AtomicBool = AtomicBool::new(false);
static SECRETS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Print every command, timestamped, before running it (the install's terminal).
pub fn echo_commands(on: bool) {
    ECHO.store(on, Ordering::Relaxed);
}

/// Strings shown as *** in anything printed or logged from now on.
pub fn set_secrets(secrets: Vec<String>) {
    // ponytail: very short secrets would mask ordinary text, so they aren't masked
    *SECRETS.lock().unwrap() = secrets.into_iter().filter(|s| s.len() >= 4).collect();
}

pub fn redact(text: &str) -> String {
    SECRETS.lock().unwrap().iter().fold(text.to_string(), |text, secret| text.replace(secret.as_str(), "***"))
}

/// "14:22:04", UTC like the ISO's clock
pub fn timestamp() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) % 86_400;
    format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

/// "[14:22:04] EXEC: mkfs.ext4 -F /dev/sda2", with the label in an ANSI color.
pub fn print_event(label: &str, color: u8, text: &str) {
    println!("\x1b[2m[{}]\x1b[0m \x1b[1;{color}m{label}:\x1b[0m {}", timestamp(), redact(text));
}

fn announce(cmd: &str, args: &str) {
    if ECHO.load(Ordering::Relaxed) {
        print_event("EXEC", 35, format!("{cmd} {args}").trim_end());
    }
}

fn announce_failure(cmd: &str, status: &ExitStatus) {
    if ECHO.load(Ordering::Relaxed) {
        print_event("FAIL", 31, &format!("{cmd} ({status})"));
    }
}

/// Defines the mode at which it is running
/// Shell in Debug does not run the actual command.
pub enum RunMode {
    Debug,
    Release,
}

/// ShellCmd is a more understandable version of command.
/// Used to represent the last run command in Shell.
pub struct ShellCmd {
    command: String,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    status: ExitStatus,
}

impl Debug for ShellCmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.status.success() {
            f.debug_struct("ShellCmd")
                .field("command", &self.command)
                .field("stdout", &self.stdout)
                .field("status", &self.status)
                .finish()
        } else {
            f.debug_struct("ShellCmd")
                .field("command", &self.command)
                .field("stderr", &self.stderr)
                .field("status", &self.status)
                .finish()
        }
    }
}

/// An abstraction over the actual shell.
/// It can run and store the results of the command.
/// Spawned commands are not stored as the result cannot be known.
/// But running any command using shell makes it easier, like building args from strings.
pub struct Shell<'a> {
    identifier: String,
    build_mode: RunMode,
    last_run_cmd: Option<ShellCmd>,
    logger: &'a Logger,
}

impl<'a> Shell<'a> {
    pub fn new<'b>(identifier: &'b str, logger: &'b Logger) -> Shell<'b> {
        Shell {
            identifier: identifier.to_string(),
            build_mode: RunMode::Release,
            last_run_cmd: None,
            logger,
        }
    }

    fn set_last_command(
        &mut self,
        command: &str,
        status: &ExitStatus,
        stdout: Option<&Vec<u8>>,
        stderr: Option<&Vec<u8>>,
    ) {
        let null_vec: Vec<u8> = Vec::<u8>::new();

        let stdout_actual = match stdout {
            Some(x) => x,
            None => &null_vec,
        };

        let stderr_actual = match stderr {
            Some(x) => x,
            None => &null_vec,
        };
        self.last_run_cmd = Some(ShellCmd {
            command: command.to_string(),
            status: status.to_owned(),
            stdout: stdout_actual.to_vec(),
            stderr: stderr_actual.to_vec(),
        });
    }

    /// Logs using the shell's logger
    pub fn log(&self, msg: &str) {
        self.logger.debug(&self.identifier, msg);
    }

    /// Run the program without stdin.
    /// Collect stdout and stderr and store it in Output.
    /// Raises error if exited with non-zero code.
    pub fn run_with_args(&mut self, cmd: &str, args: &str) -> Result<Output> {
        announce(cmd, args);
        let args_vec = shell_words::split(args)?;
        if let RunMode::Debug = &self.build_mode {
            println!("Running Shell in Test Mode: Command: {} {:#?}", cmd, args);

            let output = Command::new("echo").arg("dummy").output()?;
            self.set_last_command(cmd, &output.status, None, None);
            return Ok(output);
        }
        let output = Command::new(cmd).args(args_vec).output()?;

        if !output.status.success() {
            self.log(&format!(
                "{}: {} {:#?} failed. Exited with non-zero exit code",
                self.identifier.to_uppercase(),
                cmd,
                args
            ));
            return Err(anyhow!(
                "{}: {} failed. Exited with non-zero exit code",
                self.identifier.to_uppercase(),
                cmd
            ));
        }

        self.set_last_command(
            cmd,
            &output.status,
            Some(&output.stdout),
            Some(&output.stderr),
        );

        Ok(output)
    }


    /// Run the program without stdin inside a directory
    /// Collect stdout and stderr and store it in Output.
    /// Raises error if exited with non-zero code.
    pub fn run_with_args_in_directory(&mut self, dir: &str, cmd: &str, args: &str) -> Result<Output> {
        let args_vec = shell_words::split(args)?;
        if let RunMode::Debug = &self.build_mode {
            println!("Running Shell in Test Mode: Command: {} {:#?}", cmd, args);

            let output = Command::new("echo").current_dir(dir).arg("dummy").output()?;
            self.set_last_command(cmd, &output.status, None, None);
            return Ok(output);
        }
        let output = Command::new(cmd).args(args_vec).current_dir(dir).output()?;

        if !output.status.success() {
            self.log(&format!(
                "{}: {} {:#?} failed. Exited with non-zero exit code",
                self.identifier.to_uppercase(),
                cmd,
                args
            ));
            return Err(anyhow!(
                "{}: {} failed. Exited with non-zero exit code",
                self.identifier.to_uppercase(),
                cmd
            ));
        }

        self.set_last_command(
            cmd,
            &output.status,
            Some(&output.stdout),
            Some(&output.stderr),
        );

        Ok(output)
    }

    /// Run the program with given args without stdin.
    /// Collect stdout and stderr and store it in Output.
    /// Raises error if exited with non-zero code.
    pub fn run(&mut self, cmd: &str) -> Result<Output> {
        if let RunMode::Debug = &self.build_mode {
            println!("Running Shell in Test Mode: Command: {}", cmd);
            let output = Command::new("echo").arg("dummy").output()?;
            self.set_last_command(cmd, &output.status, None, None);
            return Ok(output);
        }
        let output = Command::new(cmd).output()?;

        if !output.status.success() {
            return Err(anyhow!(
                "{}: {} failed. Exited with non-zero exit code",
                self.identifier.to_uppercase(),
                cmd
            ));
        }

        self.set_last_command(
            cmd,
            &output.status,
            Some(&output.stdout),
            Some(&output.stderr),
        );
        Ok(output)
    }
    
    /// Run the program with given args without stdin inside a directory
    /// Collect stdout and stderr and store it in Output.
    /// Raises error if exited with non-zero code.
    pub fn run_in_directory(&mut self, dir: &str, cmd: &str) -> Result<Output> {
        if let RunMode::Debug = &self.build_mode {
            println!("Running Shell in Test Mode: Command: {}", cmd);
            let output = Command::new("echo").arg("dummy").current_dir(dir).output()?;
            self.set_last_command(cmd, &output.status, None, None);
            return Ok(output);
        }
        let output = Command::new(cmd).current_dir(dir).output()?;

        if !output.status.success() {
            return Err(anyhow!(
                "{}: {} failed. Exited with non-zero exit code",
                self.identifier.to_uppercase(),
                cmd
            ));
        }

        self.set_last_command(
            cmd,
            &output.status,
            Some(&output.stdout),
            Some(&output.stderr),
        );
        Ok(output)
    }

    /// Run the program with stdin.
    /// Only status is returned, not the output.
    /// Raises error if exited with non-zero code.
    pub fn run_and_wait(&mut self, cmd: &str) -> Result<ExitStatus> {
        announce(cmd, "");
        if let RunMode::Debug = &self.build_mode {
            println!("Running Shell in Test Mode: Command: {}", cmd);
            let output = Command::new("echo").arg("dummy").status()?;
            self.set_last_command(cmd, &output, None, None);
            return Ok(output);
        }

        let status = Command::new(cmd).status()?;

        if !status.success() {
            return Err(anyhow!(
                "{}: {} failed. Exited with non-zero exit code",
                self.identifier.to_uppercase(),
                cmd
            ));
        }

        self.set_last_command(cmd, &status, None, None);
        Ok(status)
    }


    /// Run the program with stdin inside a directory
    /// Only status is returned, not the output.
    /// Raises error if exited with non-zero code.
    pub fn run_in_directory_and_wait(&mut self, dir:&str, cmd: &str) -> Result<ExitStatus> {
        if let RunMode::Debug = &self.build_mode {
            println!("Running Shell in Test Mode: Command: {}", cmd);
            let output = Command::new("echo").current_dir(dir).status()?;
            self.set_last_command(cmd, &output, None, None);
            return Ok(output);
        }

        let status = Command::new(cmd).current_dir(dir).status()?;

        if !status.success() {
            return Err(anyhow!(
                "{}: {} failed. Exited with non-zero exit code",
                self.identifier.to_uppercase(),
                cmd
            ));
        }

        self.set_last_command(cmd, &status, None, None);
        Ok(status)
    }

    /// Run the program with given args with stdin.
    /// Only status is returned, not the output.
    /// Raises error if exited with non-zero code.
    pub fn run_and_wait_with_args(&mut self, cmd: &str, args: &str) -> Result<ExitStatus> {
        announce(cmd, args);
        let args_vec = shell_words::split(args)?;
        if let RunMode::Debug = &self.build_mode {
            println!("Running Shell in Test Mode: Command: {}", cmd);
            let status = Command::new("echo").arg("dummy").status()?;
            self.set_last_command(cmd, &status, None, None);
            return Ok(status);
        }
        let status = Command::new(cmd).args(args_vec).status()?;

        if !status.success() {
            announce_failure(cmd, &status);
            self.log(&format!("`{cmd} {args}` failed with {status}"));
            return Err(anyhow!(
                "{}: `{} {}` failed ({})",
                self.identifier.to_uppercase(),
                cmd,
                redact(args),
                status
            ));
        }

        self.set_last_command(cmd, &status, None, None);
        Ok(status)
    }

    /// Run the program with given args with stdin inside a directory
    /// Only status is returned, not the output.
    /// Raises error if exited with non-zero code.
    pub fn run_in_directory_and_wait_with_args(&mut self, dir:&str, cmd: &str, args: &str) -> Result<ExitStatus> {
        announce(cmd, args);
        let args_vec = shell_words::split(args)?;
        if let RunMode::Debug = &self.build_mode {
            println!("Running Shell in Test Mode: Command: {}", cmd);
            let status = Command::new("echo").arg("dummy").current_dir(dir).status()?;
            self.set_last_command(cmd, &status, None, None);
            return Ok(status);
        }
        let status = Command::new(cmd).args(args_vec).current_dir(dir).status()?;

        if !status.success() {
            return Err(anyhow!(
                "{}: {} failed. Exited with non-zero exit code",
                self.identifier.to_uppercase(),
                cmd
            ));
        }

        self.set_last_command(cmd, &status, None, None);
        Ok(status)
    }

    /// Run the program with given args, feeding `input` to its stdin, and wait.
    /// Raises error if exited with non-zero code. The input isn't logged (it may be a password).
    pub fn run_with_input(&mut self, cmd: &str, args: &str, input: &str) -> Result<()> {
        announce(cmd, args);
        let args_vec = shell_words::split(args)?;
        if let RunMode::Debug = &self.build_mode {
            println!("Running Shell in Test Mode: Command: {} {}", cmd, args);
            return Ok(());
        }
        let mut child = Command::new(cmd).args(args_vec).stdin(Stdio::piped()).spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes())?;
        }
        let status = child.wait()?;
        if !status.success() {
            announce_failure(cmd, &status);
            self.log(&format!("`{cmd} {args}` failed with {status}"));
            return Err(anyhow!("{}: `{} {}` failed ({})", self.identifier.to_uppercase(), cmd, redact(args), status));
        }
        self.set_last_command(cmd, &status, None, None);
        Ok(())
    }

    /// Spawn the program and do not wait for it.
    /// Return a handle to the program.
    /// Does not have access to output of the program and so does not raise any error on
    /// unsuccessful exit.
    pub fn spawn(&mut self, cmd: &str) -> Result<Child> {
        if let RunMode::Debug = &self.build_mode {
            println!("Running Shell in Test Mode: Command: {}", cmd);
            let child = Command::new("echo").arg("dummy").spawn()?;

            // spawned processes do not get saved.
            return Ok(child);
        }

        let child = Command::new(cmd).spawn()?;
        // spawned processes do not get saved.
        Ok(child)
    }
    
    /// Spawn the program and do not wait for it in directory
    /// Return a handle to the program.
    /// Does not have access to output of the program and so does not raise any error on
    /// unsuccessful exit.
    pub fn spawn_in_directory(&mut self, dir: &str, cmd: &str) -> Result<Child> {
        if let RunMode::Debug = &self.build_mode {
            println!("Running Shell in Test Mode: Command: {}", cmd);
            let child = Command::new("echo").current_dir(dir).spawn()?;

            // spawned processes do not get saved.
            return Ok(child);
        }

        let child = Command::new(cmd).current_dir(dir).spawn()?;
        // spawned processes do not get saved.
        Ok(child)
    }

    /// Spawn the program and do not wait for it.
    /// Sends the arguments to the stdin. This works like a piped input.
    /// Return a handle to the program.
    /// Does not have access to output of the program and so does not raise any error on
    /// unsuccessful exit.
    pub fn spawn_with_piped_input(&mut self, cmd: &str, input: &str) -> Result<Child> {
        if let RunMode::Debug = &self.build_mode {
            println!("Running Shell in Test Mode: Command: {}", cmd);
            let child = Command::new("echo").arg("dummy").spawn()?;

            // spawned processes do not get saved.
            return Ok(child);
        }

        let mut child = Command::new(cmd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_ref())?; // drop would happen here
        }


        Ok(child)
    }

    /// Spawn the program and do not wait for it inside a directory
    /// Sends the arguments to the stdin. This works like a piped input.
    /// Return a handle to the program.
    /// Does not have access to output of the program and so does not raise any error on
    /// unsuccessful exit.
    pub fn spawn_in_directory_with_piped_input(&mut self, dir: &str, cmd: &str, input: &str) -> Result<Child> {
        if let RunMode::Debug = &self.build_mode {
            println!("Running Shell in Test Mode: Command: {}", cmd);
            let child = Command::new("echo").arg("dummy").current_dir(dir).spawn()?;

            // spawned processes do not get saved.
            return Ok(child);
        }

        let mut child = Command::new(cmd)
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_ref())?; // drop would happen here
        }


        Ok(child)
    }

    /// Spawn the program with the given args and do not wait for it.
    /// Return a handle to the program.
    /// Does not have access to output of the program and so does not raise any error on
    /// unsuccessful exit.
    pub fn spawn_with_args(&mut self, cmd: &str, args: &str) -> Result<Child> {
        let args_vec = shell_words::split(args)?;
        if let RunMode::Debug = &self.build_mode {
            println!("Running Shell in Test Mode: Command: {}", cmd);
            let child = Command::new("echo").arg("dummy").spawn()?;
            return Ok(child);
        }
        let child = Command::new(cmd).args(args_vec).spawn()?;
        // spawned processes do not get saved.
        Ok(child)
    }

    /// Spawn the program with the given args and do not wait for it inside a directory
    /// Return a handle to the program.
    /// Does not have access to output of the program and so does not raise any error on
    /// unsuccessful exit.
    pub fn spawn_in_directory_with_args(&mut self, dir:&str, cmd: &str, args: &str) -> Result<Child> {
        let args_vec = shell_words::split(args)?;
        if let RunMode::Debug = &self.build_mode {
            println!("Running Shell in Test Mode: Command: {}", cmd);
            let child = Command::new("echo").arg("dummy").current_dir(dir).spawn()?;
            return Ok(child);
        }
        let child = Command::new(cmd).args(args_vec).current_dir(dir).spawn()?;
        // spawned processes do not get saved.
        Ok(child)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_masked() {
        set_secrets(vec!["hunter22".into(), "ab".into()]);
        assert_eq!(redact("iwctl --passphrase hunter22 station wlan0"), "iwctl --passphrase *** station wlan0");
        // too short to mask without mangling ordinary text
        assert_eq!(redact("tab"), "tab");
    }
}
