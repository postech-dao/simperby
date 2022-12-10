use path_slash::PathExt as _;

pub struct GitServer {
    child: tokio::process::Child,
    daemon_pid: u32,
}

impl GitServer {
    pub async fn join(mut self) -> Result<(), eyre::Error> {
        self.child.kill().await?;
        self.child.wait().await?;
        #[cfg(target_os = "windows")]
        {
            let mut child = tokio::process::Command::new("C:/Program Files/Git/bin/sh.exe")
                .arg("--login")
                .arg("-c")
                .arg(format!("kill {}", self.daemon_pid))
                .spawn()
                .expect("failed to kill git daemon");
            let _ = child.wait().await.expect("failed to wait on child");
        }
        #[cfg(not(target_os = "windows"))]
        {
            let mut child = tokio::process::Command::new("kill")
                .arg(format!("{}", self.daemon_pid))
                .spawn()
                .expect("failed to kill git daemon");
            let _ = child.wait().await.expect("failed to wait on child");
        }
        Ok(())
    }
}

pub async fn run_server(path: &str, port: u16) -> GitServer {
    let td = tempfile::TempDir::new().unwrap();
    let pid_path = format!("{}/pid", td.path().to_slash().unwrap().into_owned());
    let child = tokio::process::Command::new("git")
        .arg("daemon")
        .arg(format!("--base-path={}", path))
        .arg("--export-all")
        .arg(format!("--port={}", port))
        .arg(format!("--pid-file={}", pid_path))
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let daemon_pid = std::fs::read_to_string(pid_path).unwrap();
    // remove new line character
    let daemon_pid = daemon_pid[0..daemon_pid.len() - 1].parse::<u32>().unwrap();
    println!("PID: {}", daemon_pid);
    GitServer { child, daemon_pid }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simperby_test_suite::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn git_server_basic1() {
        setup_test();
        let port = dispense_port();

        let td = TempDir::new().unwrap();
        let path = td.path().to_slash().unwrap().into_owned();
        run_command(format!("cd {} && mkdir repo && cd repo && git init", path)).await;
        run_command(format!("cd {}/repo && echo 'hello' > hello.txt", path)).await;
        run_command(format!("cd {}/repo && git add -A", path)).await;
        run_command(format!(
            "cd {}/repo && git config user.name 'Test' && git config user.email 'test@test.com'",
            path
        ))
        .await;
        run_command(format!("cd {}/repo && git commit -m 'hello'", path)).await;
        let server_task = tokio::spawn(async move {
            let _x = run_server(&path, port).await;
            sleep_ms(6000).await;
        });
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        let td2 = TempDir::new().unwrap();
        let path2 = td2.path().to_slash().unwrap().into_owned();
        run_command(format!("ls {}", path2)).await;
        run_command(format!(
            "cd {} && git clone git://127.0.0.1:{}/repo",
            path2, port
        ))
        .await;
        server_task.await.unwrap();
    }
}
