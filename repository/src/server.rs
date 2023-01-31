use log::info;
use path_slash::PathExt as _;
use std::{self, os::unix::prelude::PermissionsExt, path::Path};
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

/// Runs a Simperby Git server with a push hook enabled.
///
/// - `path` is the path to the root directory of a Simperby blockchain (not the repository path)
/// - `port` is the port to run the server on
/// - `simperby_executable_path` is the path to the Simperby executable, which will be executed by the hook.
pub async fn run_server(path: &str, port: u16, simperby_executable_path: &str) -> GitServer {
    let path_hook = format!("{path}/repository/repo/.git/hooks/pre-receive");
    let hook_content = include_str!("pre_receive.sh");
    let is_hook_exist = Path::new(&path_hook).exists();
    if !is_hook_exist {
        fs::File::create(&path_hook).await.unwrap();
    }
    fs::write(&path_hook, hook_content).await.unwrap();
    fs::set_permissions(&path_hook, std::fs::Permissions::from_mode(0o755))
        .await
        .unwrap();

    let td = tempfile::TempDir::new().unwrap();
    let pid_path = format!("{}/pid", td.path().to_slash().unwrap().into_owned());

    let child = std::process::Command::new("git")
        .arg("daemon")
        .arg(format!("--base-path={path}/repository"))
        .arg("--export-all")
        .arg("--enable=receive-pack")
        .arg(format!("--port={port}"))
        .arg(format!("--pid-file={pid_path}"))
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
    use super::*;
    use crate::raw::{RawRepository, RawRepositoryImpl};
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
            "cd {path_server} && mkdir repository && cd repository && mkdir repo && cd repo && git init"
        ))
        .await;
        run_command(format!(
            "cd {path_server}/repository/repo && git config user.name 'Test' && git config user.email 'test@test.com'"
        ))
        .await;
        run_command(format!(
            "cd {path_server}/repository/repo && git config receive.advertisePushOptions true"
        ))
        .await;
        run_command(format!(
            "cd {path_server}/repository/repo && echo 'init' > init.txt && git add -A && git commit -m 'init'"
        ))
        .await;

        let td_simperby = TempDir::new().unwrap();
        let path_simperby = td_simperby.path().to_slash().unwrap().into_owned();

        // Make .sh example files for testing the server hook.
        let path_true = format!("{path_simperby}/true.sh");
        let path_false = format!("{path_simperby}/false.sh");
        fs::File::create(&path_true).await.unwrap();
        fs::File::create(&path_false).await.unwrap();
        let content_true = r#"#!/bin/sh
exit 0
"#;
        let content_false = r#"#!/bin/sh
exit 1
"#;
        fs::write(&path_true, content_true).await.unwrap();
        fs::write(&path_false, content_false).await.unwrap();
        fs::set_permissions(&path_true, std::fs::Permissions::from_mode(0o755))
            .await
            .unwrap();
        fs::set_permissions(&path_false, std::fs::Permissions::from_mode(0o755))
            .await
            .unwrap();

        // Open a git server with simperby executable which always returns true.
        let path_server_clone = path_server.to_owned();
        let server_task = tokio::spawn(async move {
            let _x = run_server(&path_server_clone, port, &path_true).await;
            sleep_ms(6000).await;
        });
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;

        // Make a local repository by cloning above server repository.
        let td_local = TempDir::new().unwrap();
        let path_local = td_local.path().to_slash().unwrap().into_owned();
        run_command(format!("ls {path_local}",)).await;
        run_command(format!(
            "cd {path_local} && git clone git://127.0.0.1:{port}/repo"
        ))
        .await;
        run_command(format!(
            "cd {path_local}/repo && git config user.name 'Test2' && git config user.email 'test2@test.com'"
        ))
        .await;
        run_command(format!(
            "cd {path_local}/repo && git branch test && git checkout test"
        ))
        .await;
        run_command(format!(
            "cd {path_local}/repo && echo 'hello' > hello.txt && git add . && git commit -m 'hello'"
        ))
        .await;

        let repo = RawRepositoryImpl::open(format!("{path_local}/repo").as_str())
            .await
            .unwrap();

        repo.push_option("origin".to_string(), "test".to_string(), None)
            .await
            .unwrap_err();

        repo.push_option(
            "origin".to_string(),
            "test".to_string(),
            Some("test".to_string()),
        )
        .await
        .unwrap();

        server_task.abort();

        // Open a git server with simperby executable which always returns false.
        let server_task = tokio::spawn(async move {
            let _x = run_server(&path_server, port, &path_false).await;
            sleep_ms(6000).await;
        });
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;

        run_command(format!(
            "cd {path_local}/repo && echo 'hello2' > hello2.txt && git add . && git commit -m 'hello2'"
        ))
        .await;

        repo.push_option("origin".to_string(), "test".to_string(), None)
            .await
            .unwrap_err();

        repo.push_option(
            "origin".to_string(),
            "test".to_string(),
            Some("test".to_string()),
        )
        .await
        .unwrap_err();

        server_task.await.unwrap();
    }
}
