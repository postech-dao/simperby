use super::SemanticCommit;
use crate::raw::Error;
use crate::raw::{CommitHash, RawCommit, RawRepository};

use simperby_common::utils::get_timestamp;
use simperby_common::{test_utils::generate_standard_genesis, Diff, ToHash256};
use std::path::Path;
use tempfile::TempDir;

const MAIN: &str = "main";
const BRANCH_A: &str = "branch_a";
const BRANCH_B: &str = "branch_b";
const TAG_A: &str = "tag_a";
const TAG_B: &str = "tag_b";

/// Make a repository which includes one initial commit at "main" branch.
/// This returns RawRepositoryImpl containing the repository.
async fn init_repository_with_initial_commit(path: &Path) -> Result<RawRepository, Error> {
    let repo = RawRepository::init(path.to_str().unwrap(), "initial", &MAIN.into())
        .await
        .unwrap();

    Ok(repo)
}

/// Initialize repository with empty commit and empty branch.
#[tokio::test]
async fn init() {
    let td = TempDir::new().unwrap();
    let path = td.path();

    let repo = RawRepository::init(path.to_str().unwrap(), "initial", &MAIN.into())
        .await
        .unwrap();
    let branch_list = repo.list_branches().await.unwrap();
    assert_eq!(branch_list, vec![MAIN.to_owned()]);

    RawRepository::init(path.to_str().unwrap(), "initial", &MAIN.into())
        .await
        .unwrap_err();
}

/// Open existed repository and verifies whether it opens well.
#[tokio::test]
async fn open() {
    let td = TempDir::new().unwrap();
    let path = td.path();

    let init_repo = init_repository_with_initial_commit(path).await.unwrap();
    let open_repo = RawRepository::open(path.to_str().unwrap()).await.unwrap();

    let branch_list_init = init_repo.list_branches().await.unwrap();
    let branch_list_open = open_repo.list_branches().await.unwrap();

    assert_eq!(branch_list_init, branch_list_open);
}

/*
   c2 (HEAD -> main)      c2 (HEAD -> main, branch_a)     c2 (HEAD -> main)
   |                -->   |                          -->  |
   c1 (branch_a)          c1                              c1
*/
/// Create "branch_a" at c1, create c2 at "main" branch and move "branch_a" head from c1 to c2.
/// Finally, "branch_a" is removed.
#[tokio::test]
async fn branch() {
    let td = TempDir::new().unwrap();
    let path = td.path();
    let mut repo = init_repository_with_initial_commit(path).await.unwrap();

    // There is one branch "main" at initial state
    let branch_list = repo.list_branches().await.unwrap();
    assert_eq!(branch_list, vec![MAIN.to_owned()]);

    // git branch branch_a
    let c1_commit_hash = repo.get_head().await.unwrap();
    repo.create_branch(BRANCH_A.into(), c1_commit_hash)
        .await
        .unwrap();

    // "branch_list" is sorted by the name of the branches in an alphabetic order
    let branch_list = repo.list_branches().await.unwrap();
    assert_eq!(branch_list, vec![BRANCH_A.to_owned(), MAIN.to_owned()]);

    let branch_a_commit_hash = repo.locate_branch(BRANCH_A.into()).await.unwrap();
    assert_eq!(branch_a_commit_hash, c1_commit_hash);

    // Make second commit with "main" branch
    let commit = RawCommit {
        message: "second".to_string(),
        diff: None,
        author: "name".to_string(),
        email: "test@email.com".to_string(),
        timestamp: get_timestamp(),
    };
    let c2_commit_hash = repo.create_commit(commit).await.unwrap();

    let branch_list_from_commit = repo.get_branches(c2_commit_hash).await.unwrap();
    assert_eq!(branch_list_from_commit, vec![MAIN.to_owned()]);

    // Move "branch_a" head to "main" head
    let main_commit_hash = repo.locate_branch(MAIN.into()).await.unwrap();
    repo.move_branch(BRANCH_A.into(), main_commit_hash)
        .await
        .unwrap();
    let branch_a_commit_hash = repo.locate_branch(BRANCH_A.into()).await.unwrap();
    assert_eq!(main_commit_hash, branch_a_commit_hash);

    let branch_list_from_commit = repo.get_branches(c2_commit_hash).await.unwrap();
    assert_eq!(
        branch_list_from_commit,
        vec![BRANCH_A.to_owned(), MAIN.to_owned()]
    );
    let branch_list_from_commit = repo.get_branches(c1_commit_hash).await.unwrap();
    assert_eq!(branch_list_from_commit.len(), 0);

    // Remove "branch_a" and the remaining branch should be only "main"
    repo.delete_branch(BRANCH_A.into()).await.unwrap();
    let branch_list = repo.list_branches().await.unwrap();
    assert_eq!(branch_list, vec![MAIN.to_owned()]);

    // This fails since current HEAD points at "main" branch
    repo.delete_branch(MAIN.into()).await.unwrap_err();
}

/*
   c1 (HEAD -> main, tag_a, tag_b)  -->  c1 (HEAD -> main, tag_b)
*/
/// Create a tag and remove it.
#[tokio::test]
async fn tag() {
    let td = TempDir::new().unwrap();
    let path = td.path();
    let mut repo = init_repository_with_initial_commit(path).await.unwrap();

    // There is no tags at initial state
    let tag_list = repo.list_tags().await.unwrap();
    assert!(tag_list.is_empty());

    // Create "tag_a" and "tag_b" at first commit
    let first_commit_hash = repo.locate_branch(MAIN.into()).await.unwrap();
    repo.create_tag(TAG_A.into(), first_commit_hash)
        .await
        .unwrap();
    repo.create_tag(TAG_B.into(), first_commit_hash)
        .await
        .unwrap();
    let tag_list = repo.list_tags().await.unwrap();
    assert_eq!(tag_list, vec![TAG_A.to_owned(), TAG_B.to_owned()]);

    let tag_a_commit_hash = repo.locate_tag(TAG_A.into()).await.unwrap();
    assert_eq!(first_commit_hash, tag_a_commit_hash);
    let tag_b_commit_hash = repo.locate_tag(TAG_B.into()).await.unwrap();
    assert_eq!(first_commit_hash, tag_b_commit_hash);

    let tags = repo.get_tag(first_commit_hash).await.unwrap();
    assert_eq!(tags, vec![TAG_A.to_owned(), TAG_B.to_owned()]);

    // Remove "tag_a"
    repo.remove_tag(TAG_A.into()).await.unwrap();
    let tag_list = repo.list_tags().await.unwrap();
    assert_eq!(tag_list, vec![TAG_B.to_owned()]);
}

/*
    c3 (HEAD -> main)   c3 (HEAD -> main)     c3 (main)                   c3 (HEAD -> main)
    |                   |                     |                           |
    c2 (branch_b)  -->  c2 (branch_b)  -->    c2 (HEAD -> branch_b)  -->  c2 (branch_b)
    |                   |                     |                           |
    c1 (branch_a)       c1 (HEAD -> branch_a) c1 (branch_a)               c1 (branch_a)
*/
/// Checkout to each commits with different branches.
#[tokio::test]
async fn checkout() {
    let td = TempDir::new().unwrap();
    let path = td.path();
    let mut repo = init_repository_with_initial_commit(path).await.unwrap();

    // Create branch_a at c1 and commit c2
    let first_commit_hash = repo.locate_branch(MAIN.into()).await.unwrap();
    repo.create_branch(BRANCH_A.into(), first_commit_hash)
        .await
        .unwrap();
    let commit = RawCommit {
        message: "second".to_string(),
        diff: None,
        author: "name".to_string(),
        email: "test@email.com".to_string(),
        timestamp: get_timestamp(),
    };
    repo.create_commit(commit).await.unwrap();
    // Create branch_b at c2 and commit c3
    let second_commit_hash = repo.locate_branch(MAIN.into()).await.unwrap();
    repo.create_branch(BRANCH_B.into(), second_commit_hash)
        .await
        .unwrap();
    let commit = RawCommit {
        message: "third".to_string(),
        diff: None,
        author: "name".to_string(),
        email: "test@email.com".to_string(),
        timestamp: get_timestamp(),
    };
    repo.create_commit(commit).await.unwrap();

    let first_commit_hash = repo.locate_branch(BRANCH_A.into()).await.unwrap();
    let second_commit_hash = repo.locate_branch(BRANCH_B.into()).await.unwrap();
    let third_commit_hash = repo.locate_branch(MAIN.into()).await.unwrap();

    // Checkout to branch_a, branch_b, main sequentially
    // Compare the head's commit hash after checkout with each branch's commit hash
    repo.checkout(BRANCH_A.into()).await.unwrap();
    let head_commit_hash = repo.get_head().await.unwrap();
    assert_eq!(head_commit_hash, first_commit_hash);
    let branch = repo.get_currently_checkout_branch().await.unwrap();
    assert_eq!(branch, Some("branch_a".to_string()));
    repo.checkout(BRANCH_B.into()).await.unwrap();
    let head_commit_hash = repo.get_head().await.unwrap();
    assert_eq!(head_commit_hash, second_commit_hash);
    let branch = repo.get_currently_checkout_branch().await.unwrap();
    assert_eq!(branch, Some("branch_b".to_string()));
    repo.checkout(MAIN.into()).await.unwrap();
    let head_commit_hash = repo.get_head().await.unwrap();
    assert_eq!(head_commit_hash, third_commit_hash);
    let branch = repo.get_currently_checkout_branch().await.unwrap();
    assert_eq!(branch, Some("main".to_string()));
}

/*
    c2 (HEAD -> main)       c2 (main)
     |                 -->   |
    c1                      c1 (HEAD)
*/
/// Checkout to commit and set "HEAD" to the detached mode.
#[tokio::test]
async fn checkout_detach() {
    let td = TempDir::new().unwrap();
    let path = td.path();
    let mut repo = init_repository_with_initial_commit(path).await.unwrap();

    // There is one branch "main" at initial state
    let branch_list = repo.list_branches().await.unwrap();
    assert_eq!(branch_list, vec![MAIN.to_owned()]);

    let first_commit_hash = repo.get_head().await.unwrap();
    // Make second commit with "main" branch
    let commit = RawCommit {
        message: "second".to_string(),
        diff: None,
        author: "name".to_string(),
        email: "test@email.com".to_string(),
        timestamp: get_timestamp(),
    };
    repo.create_commit(commit).await.unwrap();

    // Checkout to c1 and set HEAD detached mode
    repo.checkout_detach(first_commit_hash).await.unwrap();
    let branch = repo.get_currently_checkout_branch().await.unwrap();
    assert_eq!(branch, None);

    let cur_head_commit_hash = repo.get_head().await.unwrap();
    assert_eq!(cur_head_commit_hash, first_commit_hash);

    // TODO: Create a function of getting head name(see below).
    // This means the current head is at a detached mode,
    // otherwise this should be "refs/heads/main".
    //
    // let cur_head_name = repo.head().unwrap().name().unwrap().to_string();
    // assert_eq!(cur_head_name, "HEAD");
}

/// Reset the repository to the latest commit and delete any untracked files or directories.
/// Ensure that all tracked files and directories are retrieved
/// and that their contents are the same as before the reset.
#[tokio::test]
async fn checkout_clean() {
    let td = TempDir::new().unwrap();
    let mut repo = init_repository_with_initial_commit(td.path())
        .await
        .unwrap();
    let root_path = td.path().to_str().unwrap();

    // Create tracked files with new commit.
    let path = format!("{}/tracked", root_path);
    std::fs::File::create(&path).unwrap();
    std::fs::write(&path, "not modified").unwrap();
    let path = format!("{}/tracked_before_rename", root_path);
    std::fs::File::create(&path).unwrap();
    std::fs::write(&path, "before rename").unwrap();
    let path = format!("{}/tracked_directory", root_path);
    std::fs::create_dir(&path).unwrap();
    let path = format!("{}/tracked_directory/tracked", root_path);
    std::fs::File::create(&path).unwrap();
    std::fs::write(&path, "tracked").unwrap();
    let commit = RawCommit {
        message: "tracked".to_string(),
        diff: None,
        author: "name".to_string(),
        email: "test@email.com".to_string(),
        timestamp: get_timestamp(),
    };
    repo.create_commit(commit).await.unwrap();

    // Modify tracked files and create untracked files.
    let path = format!("{}/tracked", root_path);
    std::fs::write(&path, "modified").unwrap();
    let path = format!("{}/tracked_before_rename", root_path);
    let path_rename = format!("{}/tracked_after_rename", root_path);
    std::fs::rename(&path, &path_rename).unwrap();
    std::fs::write(&path_rename, "after rename").unwrap();
    let path = format!("{}/tracked_directory", root_path);
    std::fs::remove_dir_all(&path).unwrap();
    let path = format!("{}/untracked", root_path);
    std::fs::File::create(&path).unwrap();
    std::fs::write(&path, "untracked").unwrap();
    let path = format!("{}/untracked_directory", root_path);
    std::fs::create_dir(&path).unwrap();
    let path = format!("{}/untracked_directory/untracked", root_path);
    std::fs::File::create(&path).unwrap();
    std::fs::write(&path, "untracked").unwrap();

    repo.check_clean().await.unwrap_err();
    repo.checkout_clean().await.unwrap();
    repo.check_clean().await.unwrap();

    // Tracked files should exist and its contents should be same as before calling checkout_clean().
    let exist_path = [
        "tracked",
        "tracked_before_rename",
        "tracked_directory",
        "tracked_directory/tracked",
    ];
    for path in &exist_path {
        let path = td.path().join(path);
        assert!(path.exists());
    }
    let path = td.path().join("tracked");
    let contents = std::fs::read_to_string(&path).unwrap();
    assert_eq!(contents, "not modified");
    let path = td.path().join("tracked_before_rename");
    let contents = std::fs::read_to_string(&path).unwrap();
    assert_eq!(contents, "before rename");

    // Untracked files and directories should not exist.
    let not_exist_path = [
        "untracked",
        "untracked_directory",
        "untracked_directory/untracked",
        "tracked_after_rename",
    ];
    for path in &not_exist_path {
        let path = td.path().join(path);
        assert!(!path.exists());
    }
}

// Stash, apply a stash and drop a stash with tracked file.
#[tokio::test]
async fn stash() {
    let td = TempDir::new().unwrap();
    let mut repo = init_repository_with_initial_commit(td.path())
        .await
        .unwrap();

    // Create a commit with tracked file which will be used as a stash file.
    let path = td.path().join("stash_file");
    std::fs::File::create(&path).unwrap();
    std::fs::write(&path, "before modified").unwrap();
    let commit = RawCommit {
        message: "stash".to_string(),
        diff: None,
        author: "name".to_string(),
        email: "test@email.com".to_string(),
        timestamp: get_timestamp(),
    };
    repo.create_commit(commit).await.unwrap();

    // Modify a stash file and stash.
    std::fs::write(&path, "after modified").unwrap();
    repo.stash().await.unwrap();
    assert!(path.exists());
    let contents = std::fs::read_to_string(&path).unwrap();
    assert_eq!(contents, "before modified");
    // Apply a stash.
    repo.stash_apply().await.unwrap();
    assert!(path.exists());
    let contents = std::fs::read_to_string(&path).unwrap();
    assert_eq!(contents, "after modified");
    // Drop a stash and applying a stash should cause an error.
    repo.stash_drop().await.unwrap();
    repo.stash_apply().await.unwrap_err();
}

/*
    c3 (HEAD -> main)
    |
    c2
    |
    c1
*/
/// Get initial commit.
#[tokio::test]
async fn initial_commit() {
    let td = TempDir::new().unwrap();
    let path = td.path();
    let mut repo = init_repository_with_initial_commit(path).await.unwrap();

    // Create branch_a, branch_b and commits
    let first_commit_hash = repo.locate_branch(MAIN.into()).await.unwrap();
    let commit = RawCommit {
        message: "second".to_string(),
        diff: None,
        author: "name".to_string(),
        email: "test@email.com".to_string(),
        timestamp: get_timestamp(),
    };
    repo.create_commit(commit).await.unwrap();
    let commit = RawCommit {
        message: "third".to_string(),
        diff: None,
        author: "name".to_string(),
        email: "test@email.com".to_string(),
        timestamp: get_timestamp(),
    };
    repo.create_commit(commit).await.unwrap();

    let initial_commit_hash = repo.get_initial_commit().await.unwrap();
    assert_eq!(initial_commit_hash, first_commit_hash);
}

/*
    c3 (HEAD -> main)
    |
    c2
    |
    c1
*/
/// Get ancestors of c3 which are [c2, c1] in the linear commit above.
#[tokio::test]
async fn ancestor() {
    let td = TempDir::new().unwrap();
    let path = td.path();
    let mut repo = init_repository_with_initial_commit(path).await.unwrap();

    let first_commit_hash = repo.locate_branch(MAIN.into()).await.unwrap();
    // Make second and third commits at "main" branch
    let commit = RawCommit {
        message: "second".to_string(),
        diff: None,
        author: "name".to_string(),
        email: "test@email.com".to_string(),
        timestamp: get_timestamp(),
    };
    let second_commit_hash = repo.create_commit(commit).await.unwrap();
    let commit = RawCommit {
        message: "third".to_string(),
        diff: None,
        author: "name".to_string(),
        email: "test@email.com".to_string(),
        timestamp: get_timestamp(),
    };
    let third_commit_hash = repo.create_commit(commit).await.unwrap();

    // Get only one ancestor(direct parent)
    let ancestors = repo
        .list_ancestors(third_commit_hash, Some(1))
        .await
        .unwrap();
    assert_eq!(ancestors, vec![second_commit_hash]);

    // Get two ancestors with max 2
    let ancestors = repo
        .list_ancestors(third_commit_hash, Some(2))
        .await
        .unwrap();
    assert_eq!(ancestors, vec![second_commit_hash, first_commit_hash]);

    let query_path = repo
        .query_commit_path(first_commit_hash, third_commit_hash)
        .await
        .unwrap();
    assert_eq!(query_path, vec![second_commit_hash, third_commit_hash]);

    // Get all ancestors
    let ancestors = repo.list_ancestors(third_commit_hash, None).await.unwrap();
    assert_eq!(ancestors, vec![second_commit_hash, first_commit_hash]);

    // TODO: If max num > the number of ancestors
}

/*
    c3 (HEAD -> branch_b)
     |  c2 (branch_a)
     | /
    c1 (main)
*/
/// Make three commits at different branches and the merge base of (c2,c3) would be c1.
#[tokio::test]
async fn merge_base() {
    let td = TempDir::new().unwrap();
    let path = td.path();
    let mut repo = init_repository_with_initial_commit(path).await.unwrap();

    // Create "branch_a" and "branch_b" branches at c1
    {
        let commit_hash1 = repo.locate_branch(MAIN.into()).await.unwrap();
        repo.create_branch(BRANCH_A.into(), commit_hash1)
            .await
            .unwrap();
        repo.create_branch(BRANCH_B.into(), commit_hash1)
            .await
            .unwrap();
    }
    // Make a commit at "branch_a" branch
    repo.checkout(BRANCH_A.into()).await.unwrap();
    let commit = RawCommit {
        message: "branch_a".to_string(),
        diff: None,
        author: "name".to_string(),
        email: "test@email.com".to_string(),
        timestamp: get_timestamp(),
    };
    repo.create_commit(commit).await.unwrap();
    // Make a commit at "branch_b" branch
    repo.checkout(BRANCH_B.into()).await.unwrap();
    let commit = RawCommit {
        message: "branch_b".to_string(),
        diff: None,
        author: "name".to_string(),
        email: "test@email.com".to_string(),
        timestamp: get_timestamp(),
    };
    repo.create_commit(commit).await.unwrap();

    // Make merge base of (c2,c3)
    let commit_hash_main = repo.locate_branch(MAIN.into()).await.unwrap();
    let commit_hash_a = repo.locate_branch(BRANCH_A.into()).await.unwrap();
    let commit_hash_b = repo.locate_branch(BRANCH_B.into()).await.unwrap();
    let merge_base = repo
        .find_merge_base(commit_hash_a, commit_hash_b)
        .await
        .unwrap();

    // The merge base of (c2,c3) should be c1
    assert_eq!(merge_base, commit_hash_main);
}

/// TODO: Change remote repository examples.
/// Add remote repository and remove it.
#[tokio::test]
async fn remote() {
    let td = TempDir::new().unwrap();
    let path = td.path();
    let mut repo = init_repository_with_initial_commit(path).await.unwrap();

    // Add remote repositories
    repo.add_remote(
        "simperby".to_owned(),
        "https://github.com/JeongHunP/simperby.git".to_owned(),
    )
    .await
    .unwrap();
    repo.add_remote(
        "cosmos".to_owned(),
        "https://github.com/JeongHunP/cosmos.git".to_owned(),
    )
    .await
    .unwrap();

    let remote_list = repo.list_remotes().await.unwrap();
    assert_eq!(
        remote_list,
        vec![
            (
                "cosmos".to_owned(),
                "https://github.com/JeongHunP/cosmos.git".to_owned()
            ),
            (
                "simperby".to_owned(),
                "https://github.com/JeongHunP/simperby.git".to_owned()
            )
        ]
    );

    // Fetch all of the remote repositories.
    repo.fetch_all().await.unwrap();
    let branches = repo.list_remote_tracking_branches().await.unwrap();

    // Verify the commit hash of remote branch is right or not.
    let simperby_main_branch = branches
        .into_iter()
        .filter(|(remote_name, branch_name, _)| remote_name == "simperby" && branch_name == "main")
        .collect::<Vec<(String, String, CommitHash)>>();

    let simperby_main_branch_commit_hash = repo
        .locate_remote_tracking_branch("simperby".to_owned(), "main".to_owned())
        .await
        .unwrap();
    assert_eq!(simperby_main_branch[0].2, simperby_main_branch_commit_hash);

    // TODO: After read_reserved_state() implemented, add this.
    /*
    let remote_repo = git2::Repository::clone(
        "https://github.com/postech-dao/simperby-git-example.git",
        td.path().join("git-ex"),
    )
    .unwrap();
    let remote_repo = RawRepositoryImpl::open(td.path().join("git-ex").to_str().unwrap())
        .await
        .unwrap();
    let reserved_state = remote_repo.read_reserved_state().await.unwrap(); */

    // Remove "simperby" remote repository
    repo.remove_remote("simperby".to_owned()).await.unwrap();
    let remote_list = repo.list_remotes().await.unwrap();
    assert_eq!(
        remote_list,
        vec![(
            "cosmos".to_owned(),
            "https://github.com/JeongHunP/cosmos.git".to_owned()
        )]
    );
}

#[tokio::test]
async fn reserved_state() {
    let td = TempDir::new().unwrap();
    let path = td.path();
    let mut repo = init_repository_with_initial_commit(path).await.unwrap();

    let (rs, _) = generate_standard_genesis(10);

    repo.checkout(MAIN.into()).await.unwrap();
    let commit_hash = repo
        .create_semantic_commit(SemanticCommit {
            title: "test".to_owned(),
            body: "test-body".to_owned(),
            diff: Diff::Reserved(Box::new(rs.clone())),
            author: "doesn't matter".to_owned(),
            timestamp: 0,
        })
        .await
        .unwrap();
    let rs_after = repo.read_reserved_state().await.unwrap();
    let _semantic_commit = repo.read_semantic_commit(commit_hash).await.unwrap();

    assert_eq!(rs_after, rs);
}

#[tokio::test]
async fn clone() {
    let td = TempDir::new().unwrap();
    let path = td.path();

    let repo = RawRepository::clone(
        path.to_str().unwrap(),
        "https://github.com/JeongHunP/cosmos.git",
    )
    .await
    .unwrap();

    let branch_list = repo.list_branches().await.unwrap();
    assert_eq!(branch_list, vec![MAIN.to_owned()]);
}

#[tokio::test]
async fn semantic_commit() {
    let td = TempDir::new().unwrap();
    let mut repo = init_repository_with_initial_commit(td.path())
        .await
        .unwrap();
    let path = td.path().join("file");
    std::fs::File::create(&path).unwrap();
    std::fs::write(&path, "file").unwrap();
    let commit = RawCommit {
        message: "add a file".to_string(),
        diff: None,
        author: "name".to_string(),
        email: "test@email.com".to_string(),
        timestamp: get_timestamp(),
    };
    let commit_file = repo.create_commit(commit).await.unwrap();

    let semantic_commit_nonreserved = repo.read_semantic_commit(commit_file).await.unwrap();
    let patch = repo.show_commit(commit_file).await.unwrap();
    let hash = patch.to_hash256();
    assert_eq!(semantic_commit_nonreserved.diff, Diff::NonReserved(hash));
}

/*
    c3 (HEAD -> branch_b)
     |  c2 (branch_a, tag_a)
     | /
    c1 (main)
*/
/// Make three commits at different branches and retrieve commits by different revisions.
#[tokio::test]
async fn retrieve_commit_hash() {
    let td = TempDir::new().unwrap();
    let path = td.path();
    let mut repo = init_repository_with_initial_commit(path).await.unwrap();

    // Create "branch_a" and "branch_b" branches at c1
    let commit_hash_main = repo.locate_branch(MAIN.into()).await.unwrap();
    repo.create_branch(BRANCH_A.into(), commit_hash_main)
        .await
        .unwrap();
    repo.create_branch(BRANCH_B.into(), commit_hash_main)
        .await
        .unwrap();

    // Make a commit at "branch_a" branch
    repo.checkout(BRANCH_A.into()).await.unwrap();
    let commit = RawCommit {
        message: BRANCH_A.into(),
        diff: None,
        author: "name".to_string(),
        email: "test@email.com".to_string(),
        timestamp: get_timestamp(),
    };
    let commit_hash_a = repo.create_commit(commit).await.unwrap();
    // Make a tag at "branch_a" branch
    repo.create_tag(TAG_A.into(), commit_hash_a).await.unwrap();
    // Make a commit at "branch_b" branch
    repo.checkout(BRANCH_B.into()).await.unwrap();
    let commit = RawCommit {
        message: BRANCH_B.into(),
        diff: None,
        author: "name".to_string(),
        email: "test@email.com".to_string(),
        timestamp: get_timestamp(),
    };
    let commit_hash_b = repo.create_commit(commit).await.unwrap();

    // Retrieve commits by branch.
    let commit_hash_a_retrieve = repo.retrieve_commit_hash(BRANCH_A.into()).await.unwrap();
    assert_eq!(commit_hash_a_retrieve, commit_hash_a);
    let commit_hash_b_retrieve = repo.retrieve_commit_hash(BRANCH_B.into()).await.unwrap();
    assert_eq!(commit_hash_b_retrieve, commit_hash_b);
    let commit_hash_main_retrieve = repo.retrieve_commit_hash(MAIN.into()).await.unwrap();
    assert_eq!(commit_hash_main_retrieve, commit_hash_main);

    // Retrieve commits by HEAD.
    let commit_hash_head_retrieve = repo.retrieve_commit_hash("HEAD".to_owned()).await.unwrap();
    assert_eq!(commit_hash_head_retrieve, commit_hash_b);
    let commit_hash_head_retrieve = repo
        .retrieve_commit_hash("HEAD^1".to_owned())
        .await
        .unwrap();
    assert_eq!(commit_hash_head_retrieve, commit_hash_main);
    let commit_hash_head_retrieve = repo
        .retrieve_commit_hash("HEAD~1".to_owned())
        .await
        .unwrap();
    assert_eq!(commit_hash_head_retrieve, commit_hash_main);
    // This fails since there is only one parent commit.
    repo.retrieve_commit_hash("HEAD^2".to_owned())
        .await
        .unwrap_err();

    // Retrieve commits by tag.
    let commit_hash_tag_a_retrieve = repo.retrieve_commit_hash(TAG_A.into()).await.unwrap();
    assert_eq!(commit_hash_tag_a_retrieve, commit_hash_a);
}

/// Make two repositories, get patch from one repository and apply patch to the other repository.
#[tokio::test]
async fn patch() {
    // Set up two repositories.
    let td = TempDir::new().unwrap();
    let mut repo = init_repository_with_initial_commit(td.path())
        .await
        .unwrap();
    let empty_commit = RawCommit {
        message: "read_commit test".to_string(),
        diff: None,
        author: "name".to_string(),
        email: "test@email.com".to_string(),
        timestamp: get_timestamp() / 1000,
    };
    let empty_commit_hash = repo.create_commit(empty_commit.clone()).await.unwrap();
    let commit_retrieve = repo.read_commit(empty_commit_hash).await.unwrap();
    assert_eq!(empty_commit.clone(), commit_retrieve);

    let path = td.path().join("patch_file");
    std::fs::File::create(&path).unwrap();
    std::fs::write(&path, "patch test").unwrap();
    let message = "apply patch".to_string();
    let author = "name".to_string();
    let email = "test@email.com".to_string();
    let timestamp = get_timestamp() / 1000;
    let commit = RawCommit {
        message: message.clone(),
        diff: None,
        author: author.clone(),
        email: email.clone(),
        timestamp,
    };
    let patch_commit_original = repo.create_commit(commit.clone()).await.unwrap();

    let td2 = TempDir::new().unwrap();
    let path2 = td2.path();
    let mut repo2 = init_repository_with_initial_commit(path2).await.unwrap();
    repo2.create_commit(empty_commit).await.unwrap();

    let head = repo.get_head().await.unwrap();
    let patch = repo.get_patch(head).await.unwrap();
    let commit = RawCommit {
        message,
        diff: Some(patch),
        author,
        email,
        timestamp,
    };
    let patch_commit = repo2.create_commit(commit.clone()).await.unwrap();
    let commit_retrieve = repo2.read_commit(patch_commit).await.unwrap();
    assert_eq!(commit, commit_retrieve);

    assert_eq!(patch_commit_original, patch_commit);
    let patch_retrieve = repo2.get_patch(patch_commit).await.unwrap();
    // TODO: Add below lines when show_commit() is changed to return patch.
    // assert_eq!(patch, patch_retrieve);
    assert!(patch_retrieve.contains("patch_file"));
}
