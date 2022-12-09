use crate::primitives::{Storage, StorageError};
use async_trait::async_trait;
use fs2::FileExt;
use futures::stream::*;
use tokio::{fs, io::AsyncWriteExt, task::spawn_blocking};

pub struct StorageImpl {
    lock_file: Option<std::fs::File>,
    path: String,
}

#[async_trait]
impl Storage for StorageImpl {
    async fn create(storage_directory: &str) -> Result<(), StorageError> {
        let _ = fs::remove_dir_all(storage_directory).await;
        fs::create_dir(storage_directory).await?;
        fs::File::create(format!("{}/lock", storage_directory)).await?;
        Ok(())
    }

    async fn open(storage_directory: &str) -> Result<Self, StorageError>
    where
        Self: Sized,
    {
        let storage_directory_ = storage_directory.to_owned();
        let file =
            spawn_blocking(move || std::fs::File::open(format!("{}/lock", storage_directory_)))
                .await??;
        let file = spawn_blocking(move || {
            let result = file.lock_exclusive();
            result.map(|_| file)
        })
        .await??;
        Ok(Self {
            lock_file: Some(file),
            path: storage_directory.to_owned(),
        })
    }

    async fn list_files(&self) -> Result<Vec<String>, StorageError> {
        let dir = tokio_stream::wrappers::ReadDirStream::new(fs::read_dir(&self.path).await?);
        let files = dir
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        Ok(files
            .into_iter()
            .map(|file| file.file_name().into_string().unwrap())
            .filter(|file| file != "lock")
            .collect())
    }

    async fn add_or_overwrite_file(
        &mut self,
        name: &str,
        content: String,
    ) -> Result<(), StorageError> {
        let mut file = fs::File::create(format!("{}/{}", self.path, name)).await?;
        file.write_all(content.as_bytes()).await?;
        // IMPORTANT!
        file.flush().await?;
        Ok(())
    }

    async fn read_file(&self, name: &str) -> Result<String, StorageError> {
        fs::read_to_string(format!("{}/{}", self.path, name)).await
    }

    async fn remove_file(&mut self, name: &str) -> Result<(), StorageError> {
        fs::remove_file(format!("{}/{}", self.path, name)).await
    }

    async fn remove_all_files(&mut self) -> Result<(), StorageError> {
        let files = self.list_files().await?;
        for file in files {
            self.remove_file(&file).await?;
        }
        Ok(())
    }
}

impl Drop for StorageImpl {
    fn drop(&mut self) {
        let lock_file = self.lock_file.take().unwrap();
        spawn_blocking(move || {
            if let Err(e) = lock_file.unlock() {
                log::error!("failed to unlock storage: {}", e);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::*;
    use simperby_common::*;

    fn generate_random_string() -> String {
        let mut rng = rand::thread_rng();
        let s1: u128 = rng.gen();
        let s2: u128 = rng.gen();
        Hash256::hash(format!("{}{}", s1, s2).as_bytes()).to_string()[0..16].to_owned()
    }

    fn gerenate_random_storage_directory() -> String {
        let temp_dir = std::env::temp_dir();
        format!(
            "{}/{}",
            temp_dir.to_str().unwrap(),
            generate_random_string()
        )
    }

    #[tokio::test]
    async fn simple1() {
        let dir = gerenate_random_storage_directory();
        StorageImpl::create(&dir).await.unwrap();
        let mut storage = StorageImpl::open(&dir).await.unwrap();
        for _ in 0..10 {
            let name = generate_random_string();
            let content = generate_random_string();
            storage
                .add_or_overwrite_file(&name, content.clone())
                .await
                .unwrap();
            assert_eq!(storage.read_file(&name).await.unwrap(), content);
        }
    }

    #[tokio::test]
    async fn never_interrupted() {
        let dir = gerenate_random_storage_directory();
        StorageImpl::create(&dir).await.unwrap();
        let mut storage = StorageImpl::open(&dir).await.unwrap();
        let mut tasks = Vec::new();
        for _ in 0..10 {
            let name = generate_random_string();
            let content = generate_random_string();
            storage
                .add_or_overwrite_file(&name, content.clone())
                .await
                .unwrap();
        }
        for _ in 0..100 {
            let dir_ = dir.clone();
            tasks.push(tokio::spawn(async move {
                let mut storage = StorageImpl::open(&dir_).await.unwrap();
                storage.remove_all_files().await.unwrap();
            }))
        }
        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
        // assert that files are not yet removed
        assert_eq!(storage.list_files().await.unwrap().len(), 10);
        drop(storage);
        futures::future::join_all(tasks)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let storage = StorageImpl::open(&dir).await.unwrap();
        // assert that files are removed
        assert_eq!(storage.list_files().await.unwrap().len(), 0);
    }
}
