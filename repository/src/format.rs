use crate::raw::SemanticCommit;
use simperby_common::*;

pub fn to_semantic_commit(commit: &Commit, last_header: &BlockHeader) -> SemanticCommit {
    match commit {
        Commit::Agenda(agenda) => {
            let title = format!("agenda: {}/{}", last_header.height + 1, agenda.to_hash256());
            let body = serde_json::to_string(agenda).unwrap();
            SemanticCommit {
                title,
                body,
                diff: Diff::None,
            }
        }
        _ => todo!(),
    }
}

pub fn from_semantic_commit(
    _semantic_commit: SemanticCommit,
    _last_header: &BlockHeader,
) -> Result<Commit, String> {
    todo!()
}
