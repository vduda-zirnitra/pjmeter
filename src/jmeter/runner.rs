use crate::io;
use crate::project_config::ProjectConfig;
use anyhow::Result;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(not(debug_assertions))]
const JMETER_PATH: &str = "/opt/pjmeter/jmeter/bin/jmeter";

#[cfg(debug_assertions)]
const JMETER_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.sandbox/jmeter/bin/jmeter");

pub fn design_with_jmeter(
    project_config: &ProjectConfig,
    environment: &str,
    project_dir: &str,
) -> Result<()> {
    let mut cmd = init_jmeter_command(project_config, environment, project_dir);
    cmd.arg("-j")
        .arg(format!("{project_dir}/jmeter/logs/jmeter.log"));

    if let Some(default_plan) = &project_config.jmeter.default_test_plan {
        cmd.arg("-t").arg(default_plan);
    }

    let mut jmeter_process = cmd.spawn()?;
    let status = jmeter_process.wait()?;
    println!("JMeter exited with: {}", status);

    Ok(())
}

#[derive(PartialEq)]
enum ExecutionStatus {
    OK,
    Interrupted,
    Failed,
}

pub fn run_with_jmeter(
    project_config: &ProjectConfig,
    environment: &str,
    plans: Vec<String>,
    project_dir: &str,
) -> Result<()> {
    let execution_time = chrono::Local::now().format("%Y%m%dT%H%M%S").to_string();

    let plan_count = plans.len();
    for (i, plan) in plans.iter().enumerate() {
        let plan_name = io::path::clear_file_extension(plan);

        let pb = ProgressBar::new_spinner()
            .with_message(format!("[{}/{plan_count}] {plan_name}", i + 1));
        pb.set_style(ProgressStyle::with_template(
            "{spinner:.green} {msg} - {elapsed_precise} : {prefix}",
        )?);
        pb.enable_steady_tick(Duration::from_millis(80));

        let mut execution_status = ExecutionStatus::OK;

        {
            // 1 : Test plan execution
            pb.set_prefix(format!("{}", style("SETTING UP").cyan().bold()));
            let mut cmd = init_jmeter_command(project_config, environment, project_dir);
            let jtl_path = format!("{project_dir}/jmeter/results/{plan_name}_{execution_time}.jtl");
            cmd.arg("-n");
            cmd.arg("-t")
                .arg(format!("{project_dir}/jmeter/plans/{plan}"));
            cmd.arg("-l").arg(&jtl_path);
            cmd.arg("-j").arg(format!(
                "{project_dir}/jmeter/results/{plan_name}_{execution_time}.log"
            ));

            let mut jmeter_process = cmd.spawn()?;
            pb.set_prefix(format!("{}", style("RUNNING PLAN").cyan().bold()));
            // Start inactivity watchdog if configured
            let inactivity_kill_timeout_secs = project_config.jmeter.inactivity_kill_timeout;

            let (inactivity_tx, inactivity_rx) = mpsc::channel();
            let (stop_tx, stop_rx) = mpsc::channel();

            let watchdog_handle = if let Some(secs) = inactivity_kill_timeout_secs {
                if secs > 0 {
                    let jtl_path_buf = PathBuf::from(&jtl_path);
                    let timeout = Duration::from_secs(secs as u64);
                    Some(thread::spawn(move || {
                        monitor_jtl_inactivity(jtl_path_buf, timeout, inactivity_tx, stop_rx);
                    }))
                } else {
                    None
                }
            } else {
                None
            };

            let mut killed_due_to_inactivity = false;

            // Wait loop: either JMeter finishes or inactivity is detected
            let status: ExitStatus = loop {
                // If inactivity detected, kill JMeter
                if inactivity_rx.try_recv().is_ok() {
                    eprintln!(
                        "No activity detected in {} for {:?} seconds. Killing JMeter...",
                        &jtl_path,
                        inactivity_kill_timeout_secs.unwrap_or(0)
                    );
                    let _ = jmeter_process.kill(); // ignore error if already exited
                    let st = jmeter_process.wait()?;
                    killed_due_to_inactivity = true;
                    break st;
                }

                // Has JMeter exited on its own?
                if let Some(st) = jmeter_process.try_wait()? {
                    break st;
                }

                // Poll interval
                thread::sleep(Duration::from_millis(500));
            };

            // Stop the watchdog if it's still running
            let _ = stop_tx.send(());
            if let Some(handle) = watchdog_handle {
                let _ = handle.join();
            }

            // If we killed due to inactivity, run the post-kill fixup on the JTL
            if killed_due_to_inactivity {
                execution_status = ExecutionStatus::Interrupted;
                post_kill(Path::new(&jtl_path))?;
                pb.set_prefix("DONE");
            } else if !status.success() {
                execution_status = ExecutionStatus::Failed;
                pb.set_prefix(format!("{}", style("FAILED PLAN").red().bold()));
            }
        }

        // 2 : Generate report
        if execution_status != ExecutionStatus::Failed {
            pb.set_prefix(format!("{}", style("GENERATING REPORT").cyan().bold()));
            let mut cmd = init_jmeter_command(project_config, environment, project_dir);
            cmd.arg("-g").arg(format!(
                "{project_dir}/jmeter/results/{plan_name}_{execution_time}.jtl"
            ));
            cmd.arg("-o").arg(format!(
                "{project_dir}/jmeter/results/{plan_name}_{environment}_{execution_time}"
            ));

            let mut jmeter_process = cmd.spawn()?;
            let status = jmeter_process.wait()?;
            if !status.success() {
                println!("Report generation for {plan} failed: {status}");
            }
        }

        {
            // 3 : Delete temporary files
            pb.set_prefix(format!("{}", style("CLEANING").cyan().bold()));
            fs::remove_file(Path::new(
                format!("{project_dir}/jmeter/results/{plan_name}_{execution_time}.jtl").as_str(),
            ))?;
            if execution_status != ExecutionStatus::Failed {
                fs::remove_file(Path::new(
                    format!("{project_dir}/jmeter/results/{plan_name}_{execution_time}.log")
                        .as_str(),
                ))?;
            }
        }

        match execution_status {
            ExecutionStatus::OK => {
                pb.set_prefix(format!("{}", style("DONE").green().bold()));
            }
            ExecutionStatus::Interrupted => {
                pb.set_prefix(format!(
                    "{}",
                    style("DONE (interrupted due to inactivity)")
                        .yellow()
                        .bold()
                ));
            }
            ExecutionStatus::Failed => {
                pb.set_prefix(format!("{}", style("FAILED").red().bold()));
                eprintln!(
                    "Check log file {project_dir}/jmeter/results/{plan_name}_{execution_time}.log"
                );
            }
        }

        pb.finish();
    }

    Ok(())
}

fn init_jmeter_command(
    project_config: &ProjectConfig,
    environment: &str,
    project_dir: &str,
) -> Command {
    let mut cmd = Command::new(JMETER_PATH);
    cmd.current_dir(format!("{project_dir}/jmeter/plans"));
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    for (env_var, value) in project_config
        .env
        .custom_environments
        .get(environment)
        .unwrap()
    {
        // Gestion distincte entre les Strings et les autres types.
        if let Some(s) = value.as_str() {
            cmd.env(env_var, s);
        } else {
            cmd.env(env_var, value.to_string());
        }
    }

    cmd
}

fn post_kill(jtl_path: &Path) -> Result<()> {
    use std::fs::OpenOptions;

    let mut file = OpenOptions::new().read(true).write(true).open(jtl_path)?;
    let len = file.metadata()?.len();
    if len == 0 {
        return Ok(());
    }

    // Check if file ends with a newline
    let mut last_byte = [0u8; 1];
    file.seek(SeekFrom::Start(len - 1))?;
    file.read_exact(&mut last_byte)?;
    let ends_with_newline = last_byte[0] == b'\n';

    // We search backwards for the newline that precedes the last line.
    // If the file ends with '\n', we skip that final newline and look for the previous one.
    // If it doesn't, we look for the last newline in the file.
    let mut search_end_exclusive: u64 = if ends_with_newline { len - 1 } else { len };

    // Default truncation point if no newline exists is 0 (remove the only line)
    let mut trunc_to: u64 = 0;
    let mut found = false;

    // Read in small chunks from the end towards the beginning
    const CHUNK: usize = 128 * 1024; // 128 KiB
    let mut buf = vec![0u8; CHUNK];

    while search_end_exclusive > 0 {
        let to_read = std::cmp::min(search_end_exclusive, CHUNK as u64) as usize;
        let start = search_end_exclusive - to_read as u64;

        file.seek(SeekFrom::Start(start))?;
        file.read_exact(&mut buf[..to_read])?;

        // Scan backwards for '\n'
        for i in (0..to_read).rev() {
            if buf[i] == b'\n' {
                trunc_to = start + i as u64 + 1; // keep up to and including this newline
                found = true;
                break;
            }
        }

        if found {
            break;
        }

        search_end_exclusive = start;
    }

    file.set_len(trunc_to)?;
    file.sync_all()?;
    Ok(())
}

// Monitors a JTL file for growth and signals inactivity if the file size
// hasn't changed for `timeout` once the file has started receiving data.
// Stops when it receives a message on `stop_rx`.
fn monitor_jtl_inactivity(
    jtl_path: PathBuf,
    timeout: Duration,
    inactivity_tx: mpsc::Sender<()>,
    stop_rx: mpsc::Receiver<()>,
) {
    let mut last_len: Option<u64> = None;
    let mut last_change = Instant::now();

    // Polling interval for checking the file size and stop signal
    let poll_interval = Duration::from_secs(1);

    loop {
        // Stop if requested (e.g., JMeter finished)
        if stop_rx.try_recv().is_ok() {
            return;
        }

        // Probe file size
        let current_len = fs::metadata(&jtl_path).map(|m| m.len()).ok();

        match (last_len, current_len) {
            (None, Some(len)) if len > 0 => {
                // First activity detected
                last_len = Some(len);
                last_change = Instant::now();
            }
            (Some(prev), Some(curr)) => {
                if curr != prev {
                    // Activity detected -> reset inactivity timer
                    last_len = Some(curr);
                    last_change = Instant::now();
                } else {
                    // No change; check inactivity timeout
                    if last_change.elapsed() >= timeout {
                        let _ = inactivity_tx.send(());
                        return;
                    }
                }
            }
            // File doesn't exist yet or len is zero; don't start inactivity timer until first growth
            _ => {}
        }

        thread::sleep(poll_interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_post_kill_empty_file() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("empty.jtl");
        fs::write(&file_path, "").unwrap();

        let result = post_kill(&file_path);
        assert!(result.is_ok());

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "");
    }

    #[test]
    fn test_post_kill_single_line_with_newline() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("single.jtl");
        fs::write(&file_path, "line1\n").unwrap();

        post_kill(&file_path).unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "");
    }

    #[test]
    fn test_post_kill_single_line_without_newline() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("single.jtl");
        fs::write(&file_path, "line1").unwrap();

        post_kill(&file_path).unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "");
    }

    #[test]
    fn test_post_kill_multiple_lines_with_newline() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("multi.jtl");
        fs::write(&file_path, "line1\nline2\nline3\n").unwrap();

        post_kill(&file_path).unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "line1\nline2\n");
    }

    #[test]
    fn test_post_kill_multiple_lines_without_newline() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("multi.jtl");
        fs::write(&file_path, "line1\nline2\nline3").unwrap();

        post_kill(&file_path).unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "line1\nline2\n");
    }

    #[test]
    fn test_post_kill_large_file() {
        use std::io::{BufRead, BufReader};

        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("large.jtl");

        // Create a file larger than CHUNK size (128KB)
        let mut file = fs::File::create(&file_path).unwrap();
        for i in 0..10000 {
            writeln!(file, "This is line number {}", i).unwrap();
        }
        drop(file);

        // Count lines before without reading entire file into memory
        let file_before = fs::File::open(&file_path).unwrap();
        let line_count_before = BufReader::new(file_before).lines().count();

        post_kill(&file_path).unwrap();

        // Count lines after
        let file_after = fs::File::open(&file_path).unwrap();
        let line_count_after = BufReader::new(file_after).lines().count();

        assert_eq!(line_count_after, line_count_before - 1);

        // Check file ends with newline
        let metadata = fs::metadata(&file_path).unwrap();
        if metadata.len() > 0 {
            let mut file = fs::File::open(&file_path).unwrap();
            let mut last_byte = [0u8; 1];
            file.seek(SeekFrom::End(-1)).unwrap();
            file.read_exact(&mut last_byte).unwrap();
            assert_eq!(last_byte[0], b'\n');
        }
    }

    #[test]
    fn test_post_kill_two_lines() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("two.jtl");
        fs::write(&file_path, "first\nsecond\n").unwrap();

        post_kill(&file_path).unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "first\n");
    }
}
