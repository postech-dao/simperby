pub async fn run_server(path: &str, port: u16) {
    tokio::process::Command::new("git")
        .arg("daemon")
        .arg(format!("--base-path={}", path))
        .arg("--export-all")
        .arg(format!("--port={}", port))
        .spawn()
        .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use path_slash::PathExt as _;
    use simperby_test_suite::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn git_server_basic1() {
        let port = 1234;
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
        tokio::spawn(async move {
            run_server(&path, port).await;
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
    }
}
