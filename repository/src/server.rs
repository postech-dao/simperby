use log::info;
use path_slash::PathExt as _;

pub struct GitServer {
    child: std::process::Child,
    daemon_pid: u32,
}

impl Drop for GitServer {
    fn drop(&mut self) {
        info!("killing git daemon ({})..", self.daemon_pid);
        let _ = self.child.kill();
        let _ = self.child.wait();

        #[cfg(target_os = "windows")]
        {
            let mut child = std::process::Command::new("C:/Program Files/Git/bin/sh.exe")
                .arg("--login")
                .arg("-c")
                .arg(format!("kill {}", self.daemon_pid))
                .spawn()
                .expect("failed to kill git daemon");
            let _ = child.wait().expect("failed to wait on child");
        }
        #[cfg(not(target_os = "windows"))]
        {
            let mut child = std::process::Command::new("kill")
                .arg(format!("{}", self.daemon_pid))
                .spawn()
                .expect("failed to kill git daemon");
            let _ = child.wait().expect("failed to wait on child");
        }
        info!("killed git daemon ({})!", self.daemon_pid)
    }
}

/// Runs a Simperby Git server with a push hook enabled.
///
/// - `path` is the path to the root directory of a Simperby blockchain (not the repository path)
/// - `port` is the port to run the server on
/// - `simperby_executable_path` is the path to the Simperby executable, which will be executed by the hook.
pub async fn run_server(path: &str, port: u16, _simperby_executable_path: &str) -> GitServer {
    let td = tempfile::TempDir::new().unwrap();
    let pid_path = format!("{}/pid", td.path().to_slash().unwrap().into_owned());
    let child = std::process::Command::new("git")
        .arg("daemon")
        .arg(format!("--base-path={path}"))
        .arg("--export-all")
        .arg(format!("--port={port}"))
        .arg(format!("--pid-file={pid_path}"))
        .spawn()
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let daemon_pid = std::fs::read_to_string(pid_path).unwrap();
    // remove new line character
    let daemon_pid = daemon_pid[0..daemon_pid.len() - 1].parse::<u32>().unwrap();
    println!("PID: {daemon_pid}");
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
        run_command(format!("cd {path} && mkdir repo && cd repo && git init")).await;
        run_command(format!("cd {path}/repo && echo 'hello' > hello.txt")).await;
        run_command(format!("cd {path}/repo && git add -A")).await;
        run_command(format!(
            "cd {path}/repo && git config user.name 'Test' && git config user.email 'test@test.com'"
        ))
        .await;
        run_command(format!("cd {path}/repo && git commit -m 'hello'")).await;
        let server_task = tokio::spawn(async move {
            let _x = run_server(&path, port, "").await;
            sleep_ms(6000).await;
        });
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        let td2 = TempDir::new().unwrap();
        let path2 = td2.path().to_slash().unwrap().into_owned();
        run_command(format!("ls {path2}")).await;
        run_command(format!(
            "cd {path2} && git clone git://127.0.0.1:{port}/repo"
        ))
        .await;
        server_task.await.unwrap();
    }
}
