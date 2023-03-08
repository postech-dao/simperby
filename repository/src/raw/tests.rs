use super::SemanticCommit;
use crate::raw::Error;
use crate::raw::{CommitHash, RawRepository, RawRepositoryImpl};

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
async fn init_repository_with_initial_commit(path: &Path) -> Result<RawRepositoryImpl, Error> {
    let repo = RawRepositoryImpl::init(path.to_str().unwrap(), "initial", &MAIN.into())
        .await
        .unwrap();

    Ok(repo)
}

/// Initialize repository with empty commit and empty branch.
#[tokio::test]
async fn init() {
    let td = TempDir::new().unwrap();
    let path = td.path();

    let repo = RawRepositoryImpl::init(path.to_str().unwrap(), "initial", &MAIN.into())
        .await
        .unwrap();
    let branch_list = repo.list_branches().await.unwrap();
    assert_eq!(branch_list, vec![MAIN.to_owned()]);

    RawRepositoryImpl::init(path.to_str().unwrap(), "initial", &MAIN.into())
        .await
        .unwrap_err();
}

/// Open existed repository and verifies whether it opens well.
#[tokio::test]
async fn open() {
    let td = TempDir::new().unwrap();
    let path = td.path();

    let init_repo = init_repository_with_initial_commit(path).await.unwrap();
    let open_repo = RawRepositoryImpl::open(path.to_str().unwrap())
        .await
        .unwrap();

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
    let c2_commit_hash = repo
        .create_commit(
            "second".to_owned(),
            "name".to_string(),
            "test@email.com".to_string(),
            get_timestamp(),
            None,
        )
        .await
        .unwrap();

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
    repo.create_commit(
        "second".to_owned(),
        "name".to_string(),
        "test@email.com".to_string(),
        get_timestamp(),
        None,
    )
    .await
    .unwrap();
    // Create branch_b at c2 and commit c3
    let second_commit_hash = repo.locate_branch(MAIN.into()).await.unwrap();
    repo.create_branch(BRANCH_B.into(), second_commit_hash)
        .await
        .unwrap();
    repo.create_commit(
        "third".to_owned(),
        "name".to_string(),
        "test@email.com".to_string(),
        get_timestamp(),
        None,
    )
    .await
    .unwrap();

    let first_commit_hash = repo.locate_branch(BRANCH_A.into()).await.unwrap();
    let second_commit_hash = repo.locate_branch(BRANCH_B.into()).await.unwrap();
    let third_commit_hash = repo.locate_branch(MAIN.into()).await.unwrap();

    // Checkout to branch_a, branch_b, main sequentially
    // Compare the head's commit hash after checkout with each branch's commit hash
    repo.checkout(BRANCH_A.into()).await.unwrap();
    let head_commit_hash = repo.get_head().await.unwrap();
    assert_eq!(head_commit_hash, first_commit_hash);
    repo.checkout(BRANCH_B.into()).await.unwrap();
    let head_commit_hash = repo.get_head().await.unwrap();
    assert_eq!(head_commit_hash, second_commit_hash);
    repo.checkout(MAIN.into()).await.unwrap();
    let head_commit_hash = repo.get_head().await.unwrap();
    assert_eq!(head_commit_hash, third_commit_hash);
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
    repo.create_commit(
        "second".to_owned(),
        "name".to_string(),
        "test@email.com".to_string(),
        get_timestamp(),
        None,
    )
    .await
    .unwrap();

    // Checkout to c1 and set HEAD detached mode
    repo.checkout_detach(first_commit_hash).await.unwrap();

    let cur_head_commit_hash = repo.get_head().await.unwrap();
    assert_eq!(cur_head_commit_hash, first_commit_hash);

    // TODO: Create a function of getting head name(see below).
    // This means the current head is at a detached mode,
    // otherwise this should be "refs/heads/main".
    //
    // let cur_head_name = repo.head().unwrap().name().unwrap().to_string();
    // assert_eq!(cur_head_name, "HEAD");
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
    repo.create_commit(
        "second".to_owned(),
        "name".to_string(),
        "test@email.com".to_string(),
        get_timestamp(),
        None,
    )
    .await
    .unwrap();
    repo.create_commit(
        "third".to_owned(),
        "name".to_string(),
        "test@email.com".to_string(),
        get_timestamp(),
        None,
    )
    .await
    .unwrap();

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
    let second_commit_hash = repo
        .create_commit(
            "second".to_owned(),
            "name".to_string(),
            "test@email.com".to_string(),
            get_timestamp(),
            None,
        )
        .await
        .unwrap();
    let third_commit_hash = repo
        .create_commit(
            "third".to_owned(),
            "name".to_string(),
            "test@email.com".to_string(),
            get_timestamp(),
            None,
        )
        .await
        .unwrap();

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
    let _commit = repo
        .create_commit(
            "branch_a".to_owned(),
            "name".to_string(),
            "test@email.com".to_string(),
            get_timestamp(),
            None,
        )
        .await
        .unwrap();
    // Make a commit at "branch_b" branch
    repo.checkout(BRANCH_B.into()).await.unwrap();
    let _commit = repo
        .create_commit(
            "branch_b".to_owned(),
            "name".to_string(),
            "test@email.com".to_string(),
            get_timestamp(),
            None,
        )
        .await
        .unwrap();

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

    let repo = RawRepositoryImpl::clone(
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
    let commit_file = repo
        .create_commit(
            "add a file".to_string(),
            "name".to_string(),
            "test@email.com".to_string(),
            get_timestamp(),
            None,
        )
        .await
        .unwrap();

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
    let commit_hash_a = repo
        .create_commit(
            BRANCH_A.into(),
            "name".to_string(),
            "test@email.com".to_string(),
            get_timestamp(),
            None,
        )
        .await
        .unwrap();
    // Make a tag at "branch_a" branch
    repo.create_tag(TAG_A.into(), commit_hash_a).await.unwrap();
    // Make a commit at "branch_b" branch
    repo.checkout(BRANCH_B.into()).await.unwrap();
    let commit_hash_b = repo
        .create_commit(
            BRANCH_B.into(),
            "name".to_string(),
            "test@email.com".to_string(),
            get_timestamp(),
            None,
        )
        .await
        .unwrap();

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
    let path = td.path().join("patch_file");
    std::fs::File::create(&path).unwrap();
    std::fs::write(&path, "patch test").unwrap();
    let commit_message = "apply patch".to_string();
    let author_name = "name".to_string();
    let author_email = "test@email.com".to_string();
    let timestamp = get_timestamp();
    let patch_commit_original = repo
        .create_commit(
            commit_message.clone(),
            author_name.clone(),
            author_email.clone(),
            timestamp,
            None,
        )
        .await
        .unwrap();

    let td2 = TempDir::new().unwrap();
    let path2 = td2.path();
    let mut repo2 = init_repository_with_initial_commit(path2).await.unwrap();

    let head = repo.get_head().await.unwrap();
    let patch = repo.get_patch(head).await.unwrap();
    let patch_commit = repo2
        .create_commit(
            commit_message,
            author_name,
            author_email,
            timestamp,
            Some(patch),
        )
        .await
        .unwrap();

    assert_eq!(patch_commit_original, patch_commit);
    let patch_retrieve = repo2.get_patch(patch_commit).await.unwrap();
    // TODO: Add below lines when show_commit() is changed to return patch.
    // assert_eq!(patch, patch_retrieve);
    assert!(patch_retrieve.contains("patch_file"));
}
