use async_trait::async_trait;

pub type StorageError = std::io::Error;

/// An abstraction of the synchronized storage backed by the host file system.
#[async_trait]
pub trait Storage: Send + Sync + 'static {
    /// Creates a new and empty directory.
    /// If there is already a directory, it just removes it and re-create.
    async fn create(storage_directory: &str) -> Result<(), StorageError>;

    /// Opens an existing directory, locking it.
    async fn open(storage_directory: &str) -> Result<Self, StorageError>
    where
        Self: Sized;

    /// Shows the list of files.
    async fn list_files(&self) -> Result<Vec<String>, StorageError>;

    /// Adds the given file to the storage.
    async fn add_or_overwrite_file(
        &mut self,
        name: &str,
        content: String,
    ) -> Result<(), StorageError>;

    /// Reads the given file.
    async fn read_file(&self, name: &str) -> Result<String, StorageError>;

    /// Removes the given file.
    async fn remove_file(&mut self, name: &str) -> Result<(), StorageError>;

    /// Removes all files.
    async fn remove_all_files(&mut self) -> Result<(), StorageError>;
}
