use super::*;
use raw::RawCommit;
use simperby_network::Error;
use simperby_network::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayloadCommit {
    pub commit: RawCommit,
    pub hash: CommitHash,
    pub parent_hash: CommitHash,
    // 순서를 정렬하려면 브랜치 별로 구분하기 위한 고유 식별자와 index가 필요함
    pub branch_name: String,
    pub index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchType {
    Agenda,
    AgendaProof,
    Block,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayloadBranch {
    pub branch_type: BranchType,
    /// The list of commit hashes in the branch, starting from
    /// **the next commit** of the `finalized` commit.
    pub commit_hashes: Vec<CommitHash>,
    /// 브랜치를 만들 때 필요한 이름을 표시하기 위해 사용.
    pub branch_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Message {
    Commit(PayloadCommit),
    Branch(PayloadBranch),
}

impl ToHash256 for Message {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_spb::to_vec(self).unwrap())
    }
}

impl DmsMessage for Message {
    fn check(&self) -> Result<(), Error> {
        Ok(())
    }
}

/// branch를 dms로 전송하는 부분이랑 dms를 받아오는 부분을 각각 flush, update 함수에서 구현을 하고 있는 상태
/*
   flush에서는 PayloadCommit랑 PayloadBranch를 만들어서 보내는 부분이 둘 다 있어야 함.
   나중에 peer가 dms 를 받을 때 순서를 보장 받지 못함 -> 순서 매칭을 payloadBranch로, 내용을 payloadCommit으로 보내는 방식으로 구현.
*/

pub async fn flush(
    _raw: Arc<RwLock<RawRepository>>,
    _dms: Arc<RwLock<Dms<Message>>>,
) -> Result<(), Error> {
    // flush에서는 클라이언트가 _raw에서부터 브랜치를 모두 불러온 다음에
    // a-, b-의 경우를 나눠서 BranchType:: 을 지정하고 dms로 전송할 수 있도록 함.
    let last_finalized_commit_hash = _raw
        .write()
        .await
        .locate_branch(FINALIZED_BRANCH_NAME.into())
        .await?;

    let branches = _raw
        .write()
        .await
        .get_branches(last_finalized_commit_hash)
        .await?;

    for branch in branches {
        if branch.starts_with("a-") {
            let a_commit_hash = _raw.write().await.locate_branch(branch.clone()).await?;
            let commit_hashes = _raw
                .write()
                .await
                .query_commit_path(a_commit_hash, last_finalized_commit_hash)
                .await?;
            let payload_branch = PayloadBranch {
                branch_type: BranchType::Agenda,
                commit_hashes: commit_hashes.clone(),
                branch_name: branch.clone(),
            };
            let message = Message::Branch(payload_branch);
            _dms.write().await.commit_message(&message).await?;

            // 위에서 PayloadBranch를 commit_message로 보내는 것처럼 PayloadCommit에 대해 commit_message을 보내야함
            // query_commit_path가 descendant를 include하는데, 그러면 last_finalized_commit_hash도 받아오는건가?
            let mut prev_commit = last_finalized_commit_hash;
            for (_index, commit_hash) in commit_hashes.iter().enumerate() {
                let payload_commit = PayloadCommit {
                    commit: _raw.write().await.read_commit(*commit_hash).await?,
                    hash: *commit_hash,
                    parent_hash: prev_commit,
                    branch_name: branch.clone(),
                    index: _index,
                };
                prev_commit = *commit_hash;

                let message = Message::Commit(payload_commit);
                _dms.write().await.commit_message(&message).await?;
                // commit_message에서 vec<PayloadCommit>을 한번에 보낼 수 없다면, commit들을 하나하나 보낸 다음에 peer가 그거 받아서 재조립해야되는데 씹재앙
                // Message Type으로 만들어서 한번에 보내는건 안되나?
            }
        } else if branch.starts_with("b-") {
            let b_commit_hash = _raw.write().await.locate_branch(branch.clone()).await?;
            let commit_hashes = _raw
                .write()
                .await
                .query_commit_path(b_commit_hash, last_finalized_commit_hash)
                .await?;
            let payload_branch = PayloadBranch {
                branch_type: BranchType::Block,
                commit_hashes: commit_hashes.clone(),
                branch_name: branch.clone(),
            };
            let message = Message::Branch(payload_branch);
            _dms.write().await.commit_message(&message).await?;

            let mut prev_commit = last_finalized_commit_hash;
            for (_index, commit_hash) in commit_hashes.iter().enumerate() {
                let payload_commit = PayloadCommit {
                    commit: _raw.write().await.read_commit(*commit_hash).await?,
                    hash: *commit_hash,
                    parent_hash: prev_commit,
                    branch_name: branch.clone(),
                    index: _index,
                };
                prev_commit = *commit_hash;

                let message = Message::Commit(payload_commit);
                _dms.write().await.commit_message(&message).await?;
            }
        } else {
            continue;
        }
    }

    // filtering을 해서 a-~ b-~ 이 두 가지 경우를 제외하고는 아무것도 안하고 그 두 경우에는 encoding해서 send한다.

    todo!()
}

/// Updates the repository module with the latest messages from the DMS.
///
/// Note that it never finalizes a block.
/// Finalization is done by the consensus module, or the `sync` method.
/*
   예외처리 & Detail
   1. (First Commit hash of received things) == (local repo's final hash)
   2. payload commit들 순서 보장하는거 -> 근데 걍 이거 Message type을 하나 더 만드는게 나을 것 같은데
   3. PayloadBranch가 아직 도착하지 않았는데 PayloadCommit vector들이 먼저 도착 -> 순서가 올 때까지 기다려야함
       -> commit hash값으로 알 수 있을 것 같음
   4.
*/

// 근데 걍 PayloadCommitSequence를 보내면 안되나?
pub async fn update(
    _raw: Arc<RwLock<RawRepository>>,
    _dms: Arc<RwLock<Dms<Message>>>,
    _commit_buffer: &mut std::collections::HashMap<String, Vec<PayloadCommit>>,
) -> Result<(), Error> {
    let messages = _dms.write().await.read_messages().await?;
    let last_message = messages.last().unwrap().clone().message;

    // 확인해야 할 부분 :
    // semantic commit을 만들어서 raw에 업데이트 하는 과정을 for문 안에서 create_semantic_commit을 하는 식으로 처리했는데,
    // 이렇게 하는 방식이 맞는가?
    match last_message {
        Message::Branch(payloadbranch) => {
            let commit_hashes = payloadbranch.commit_hashes; // sender가 보낸 commit hash들
            let branch_name = payloadbranch.branch_name; // sender의 branch-name을 그대로 이용

            _raw.write()
                .await
                .checkout(FINALIZED_BRANCH_NAME.into())
                .await?;

            let last_finalized_commit_hash = _raw
                .write()
                .await
                .locate_branch(FINALIZED_BRANCH_NAME.into())
                .await?;

            _raw.write()
                .await
                .create_branch(branch_name.clone(), last_finalized_commit_hash)
                .await?;

            for commit in commit_hashes {
                // 모든 받아온 commit hash들에 대해서 semantic commit을 만들어서 local repo를 업데이트 시킨다.
                // 이 부분은 어떤 다른 함수를 써야 되는지 모르겠어서 못건들였음
                let semantic_commit = _raw.write().await.read_semantic_commit(commit).await?;
                _raw.write()
                    .await
                    .create_semantic_commit(semantic_commit)
                    .await?;
            }
        }
        Message::Commit(payloadcommit) => {
            let branch_name = payloadcommit.branch_name.clone();
            let index = payloadcommit.index;

            // PayloadCommit Reassembling
            if let Some(vec) = _commit_buffer.get_mut(&branch_name).as_mut() {
                vec[index] = payloadcommit.clone();
            } else {
                let mut vec: Vec<PayloadCommit> = Vec::new();
                vec[index] = payloadcommit.clone();
                _commit_buffer.insert(branch_name.clone(), vec);
            }

            let last_finalized_commit_hash = _raw
                .write()
                .await
                .locate_branch(FINALIZED_BRANCH_NAME.into())
                .await?;

            let branches = _raw
                .write()
                .await
                .get_branches(last_finalized_commit_hash)
                .await?;

            for branch in branches {
                let end_commit_hash = _raw.write().await.locate_branch(branch.clone()).await?;
                let bn_commit_hashes = _raw
                    .write()
                    .await
                    .query_commit_path(end_commit_hash, last_finalized_commit_hash)
                    .await?;
                let commit_series = _commit_buffer.get(&branch).unwrap();

                // PayloadBranch와 PayloadCommitSequence의 길이가 다르면 아직 덜 도착한 것이므로 continue
                if bn_commit_hashes.len() != commit_series.len() {
                    continue;
                }
                // 개수가 맞다면 대략 다 도착한 것이므로, 순서 맞는지와 parent관계 맞는지 확인
                // 처음에 Finalized hash value부터 비교
                if last_finalized_commit_hash != commit_series[0].parent_hash
                    || bn_commit_hashes[0] != commit_series[0].hash
                {
                    continue;
                }
                //repo에 적용할 준비가 되었는지 확인하는 변수
                let is_ready =
                    bn_commit_hashes
                        .iter()
                        .enumerate()
                        .skip(1)
                        .all(|(_index, commit_hash)| {
                            *commit_hash == commit_series[_index].hash
                                && bn_commit_hashes[_index - 1] == commit_series[_index].parent_hash
                        });

                if is_ready {
                    // 지금 commit_series에는 순서대로 commit들이 들어있다는 거
                    // 이제 이걸로 semantic commit을 만들어서 local repo를 업데이트 시키면 됨
                    // 이후에 hashmap 비워주면 됨
                    for commit in commit_series {
                        let semantic_commit =
                            _raw.write().await.read_semantic_commit(commit.hash).await?;
                        _raw.write()
                            .await
                            .create_semantic_commit(semantic_commit)
                            .await?;
                    }
                    _commit_buffer.remove(&branch);
                }
            }
            todo!()
        }
    };

    todo!()
}
