#[derive(Debug, thiserror::Error)]
pub enum CollabError {
  #[error(transparent)]
  SerdeJson(#[from] serde_json::Error),

  #[error("Unexpected empty value")]
  UnexpectedEmpty,

  #[error("Get write txn failed")]
  AcquiredWriteTxnFail,

  #[error("Try apply update failed: {0}")]
  YrsTransactionError(String),

  #[error("UndoManager is not enabled")]
  UndoManagerNotEnabled,

  #[error(transparent)]
  DecodeUpdate(#[from] yrs::encoding::read::Error),

  #[error(transparent)]
  Awareness(#[from] crate::core::awareness::Error),

  #[error("Internal failure: {0}")]
  Internal(#[from] Box<dyn std::error::Error + Send + Sync>),
}
