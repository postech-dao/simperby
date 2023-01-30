use log::info;
use path_slash::PathExt as _;
use std;
use tokio::fs;

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

pub async fn run_server_legacy(path: &str, port: u16) -> GitServer {
    let td = tempfile::TempDir::new().unwrap();
    let pid_path = format!("{}/pid", td.path().to_slash().unwrap().into_owned());
    let child = std::process::Command::new("git")
        .arg("daemon")
        .arg(format!("--base-path={}", path))
        .arg("--export-all")
        .arg(format!("--port={}", port))
        .arg(format!("--pid-file={}", pid_path))
        .spawn()
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let daemon_pid = std::fs::read_to_string(pid_path).unwrap();
    // remove new line character
    let daemon_pid = daemon_pid[0..daemon_pid.len() - 1].parse::<u32>().unwrap();
    println!("PID: {}", daemon_pid);
    GitServer { child, daemon_pid }
}

/// Runs a Simperby Git server with a push hook enabled.
///
/// - `path` is the path to the root directory of a Simperby blockchain (not the repository path)
/// - `port` is the port to run the server on
/// - `simperby_executable_path` is the path to the Simperby executable, which will be executed by the hook.
pub async fn run_server(path: &str, port: u16, simperby_executable_path: &str) -> GitServer {
    fs::rename(
        format!("{}/repository/repo/.git/hooks/pre-receive.sample", path),
        format!("{}/repository/repo/.git/hooks/pre-receive", path),
    )
    .await
    .unwrap();

    // TODO: pre_receive.sh should be modified after Simperby executable is made.
    let hook_content = include_str!("pre_receive.sh");
    fs::write(
        format!("{}/repository/repo/.git/hooks/pre-receive", path),
        hook_content,
    )
    .await
    .unwrap();

    let td = tempfile::TempDir::new().unwrap();
    let pid_path = format!("{}/pid", td.path().to_slash().unwrap().into_owned());

    let child = std::process::Command::new("git")
        .arg("daemon")
        .arg(format!("--base-path={}/repository", path))
        .arg("--export-all")
        .arg(format!("--port={}", port))
        .arg(format!("--pid-file={}", pid_path))
        .env("SIMPERBY_PATH", simperby_executable_path)
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
    use crate::raw::{RawRepository, RawRepositoryImpl};

    use super::*;
    use simperby_test_suite::*;
    use std::{self, os::unix::prelude::PermissionsExt};
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
            let _x = run_server_legacy(&path, port).await;
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

    #[tokio::test]
    async fn git_server_basic2() {
        setup_test();
        let port = dispense_port();

        // Make a git repository.
        let td_server = TempDir::new().unwrap();
        let path_server = td_server.path().to_slash().unwrap().into_owned();

        run_command(format!(
            "cd {} && mkdir repository && cd repository && mkdir repo && cd repo && git init",
            path_server
        ))
        .await;
        run_command(format!(
            "cd {}/repository/repo && git config user.name 'Test' && git config user.email 'test@test.com'",
            path_server
        ))
        .await;

        // TODO: change config.
        run_command(format!(
            "cd {}/repository/repo && git config receive.advertisePushOptions true",
            path_server
        ))
        .await;
        run_command(format!(
            "cd {}/repository/repo && echo 'init' > init.txt",
            path_server
        ))
        .await;
        run_command(format!("cd {}/repository/repo && git add -A", path_server)).await;
        run_command(format!(
            "cd {}/repository/repo && git commit -m 'init'",
            path_server
        ))
        .await;

        let td_simperby = TempDir::new().unwrap();
        let simperby_executable_path = td_simperby.path().to_slash().unwrap().into_owned();

        // Make .sh example file for testing the server hook.
        let path_cli = format!("{}/simperby_cli_example.sh", simperby_executable_path);
        fs::File::create(&path_cli).await.unwrap();
        let cli_content = r#"#!/bin/sh
string=$1
result=true
case "$string" in
reject)
    result=false
    ;;
esac

echo "$result"
"#;
        fs::write(&path_cli, cli_content).await.unwrap();
        fs::set_permissions(&path_cli, std::fs::Permissions::from_mode(0o755))
            .await
            .unwrap();

        let server_task = tokio::spawn(async move {
            let _x = run_server(&path_server, port, &simperby_executable_path).await;
            sleep_ms(6000).await;
        });
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;

        // Make a local repository by cloning above server repository.
        let td_local = TempDir::new().unwrap();
        let path_local = td_local.path().to_slash().unwrap().into_owned();
        run_command(format!("ls {}", path_local)).await;
        run_command(format!(
            "cd {} && git clone git://127.0.0.1:{}/repo",
            path_local, port
        ))
        .await;
        run_command(format!(
            "cd {}/repo && git config user.name 'Test2' && git config user.email 'test2@test.com'",
            path_local
        ))
        .await;
        run_command(format!("cd {}/repo && git branch test", path_local)).await;
        run_command(format!("cd {}/repo && git checkout test", path_local)).await;

        run_command(format!(
            "cd {}/repo && echo 'hello' > hello.txt",
            path_local
        ))
        .await;
        run_command(format!("cd {}/repo && git add .", path_local)).await;
        run_command(format!(
            r#"cd {}/repo && git commit -m "hello""#,
            path_local
        ))
        .await;

        // Push with the string which is unacceptable to the server hook.
        let repo = RawRepositoryImpl::open(format!("{}/repo", path_local).as_str())
            .await
            .unwrap();

        let result = repo
            .push_option(
                "origin".to_string(),
                "test".to_string(),
                Some("reject".to_string()),
            )
            .await
            .unwrap();
        assert!(!result);

        let result = repo
            .push_option("origin".to_string(), "test".to_string(), None)
            .await
            .unwrap();
        assert!(!result);

        // Push with the string which is acceptable to the server hook.
        let result = repo
            .push_option(
                "origin".to_string(),
                "test".to_string(),
                Some("accept".to_string()),
            )
            .await
            .unwrap();
        assert!(result);

        server_task.await.unwrap();
    }
}
