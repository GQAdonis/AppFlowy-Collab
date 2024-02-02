use collab::core::collab::{CollabDocState, MutexCollab};
use collab::util::af_spawn;
use collab_entity::CollabType;
use collab_plugins::local_storage::CollabPersistenceConfig;
use collab_plugins::CollabKVDB;
use lru::LruCache;
use parking_lot::Mutex;

use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Weak};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::blocks::task_controller::{BlockTask, BlockTaskController};
use crate::rows::{
  meta_id_from_row_id, Cell, DatabaseRow, MutexDatabaseRow, Row, RowChangeSender, RowDetail, RowId,
  RowMeta, RowMetaKey, RowMetaUpdate, RowUpdate,
};
use crate::user::DatabaseCollabService;
use crate::views::RowOrder;

#[derive(Clone)]
pub enum BlockEvent {
  /// The Row is fetched from the remote.
  DidFetchRow(Vec<RowDetail>),
}

/// Each [Block] contains a list of [DatabaseRow]s. Each [DatabaseRow] represents a row in the database.
/// Currently, we only use one [Block] to manage all the rows in the database. In the future, we
/// might want to split the rows into multiple [Block]s to improve performance.
#[derive(Clone)]
pub struct Block {
  uid: i64,
  collab_db: Weak<CollabKVDB>,
  collab_service: Arc<dyn DatabaseCollabService>,
  task_controller: Arc<BlockTaskController>,
  sequence: Arc<AtomicU32>,
  pub cache: Arc<Mutex<LruCache<RowId, Arc<MutexDatabaseRow>>>>,
  pub notifier: Arc<broadcast::Sender<BlockEvent>>,
  row_change_tx: Option<RowChangeSender>,
}
unsafe impl Send for Block {}
unsafe impl Sync for Block {}
impl Block {
  pub fn new(
    uid: i64,
    collab_db: Weak<CollabKVDB>,
    collab_service: Arc<dyn DatabaseCollabService>,
    row_change_tx: Option<RowChangeSender>,
  ) -> Block {
    let cache = Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap())));
    let controller = BlockTaskController::new(collab_db.clone(), Arc::downgrade(&collab_service));
    let task_controller = Arc::new(controller);
    let (notifier, _) = broadcast::channel(1000);
    Self {
      uid,
      collab_db,
      cache,
      task_controller,
      collab_service,
      sequence: Arc::new(Default::default()),
      notifier: Arc::new(notifier),
      row_change_tx,
    }
  }

  pub fn subscribe_event(&self) -> broadcast::Receiver<BlockEvent> {
    self.notifier.subscribe()
  }

  pub fn batch_load_rows(&self, row_ids: Vec<RowId>) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    self.task_controller.add_task(BlockTask::BatchFetchRow {
      uid: self.uid,
      row_ids,
      seq: self.sequence.fetch_add(1, Ordering::SeqCst),
      sender: tx,
    });

    let weak_notifier = Arc::downgrade(&self.notifier);
    af_spawn(async move {
      while let Some(row_details) = rx.recv().await {
        if let Some(notifier) = weak_notifier.upgrade() {
          let _ = notifier.send(BlockEvent::DidFetchRow(row_details));
        }
      }
    });
  }

  pub fn close_rows(&self, row_ids: &[RowId]) {
    let mut cache_guard = self.cache.lock();
    for row_id in row_ids {
      cache_guard.pop(row_id);
    }
  }

  pub fn create_rows<T: Into<Row>>(&self, rows: Vec<T>) -> Vec<RowOrder> {
    let mut row_orders: Vec<RowOrder> = vec![];
    for row in rows.into_iter() {
      let row_order = self.create_row(row);
      row_orders.push(row_order);
    }
    row_orders
  }

  pub fn create_row<T: Into<Row>>(&self, row: T) -> RowOrder {
    let row = row.into();
    let row_id = row.id.clone();
    let row_order = RowOrder {
      id: row.id.clone(),
      height: row.height,
    };

    let collab = self.collab_for_row(&row_id);
    let database_row = MutexDatabaseRow::new(DatabaseRow::create(
      row,
      self.uid,
      row_id.clone(),
      self.collab_db.clone(),
      collab,
      self.row_change_tx.clone(),
    ));
    self.cache.lock().put(row_id, Arc::new(database_row));
    row_order
  }

  /// If the row with given id exists, return it. Otherwise, return an empty row with given id.
  /// An empty [Row] is a row with no cells.
  pub async fn get_row(&self, row_id: &RowId) -> Row {
    if let Some(mutex_row) = self.get_or_init_row(row_id).await {
      if let Some(row) = mutex_row.lock().await.get_row() {
        return row;
      }
    }

    Row::empty(row_id.clone())
  }

  pub async fn get_row_meta(&self, row_id: &RowId) -> Option<RowMeta> {
    match self
      .get_or_init_row(row_id)
      .await?
      .lock()
      .await
      .get_row_meta()
    {
      None => Some(RowMeta::empty()),
      Some(meta) => Some(meta),
    }
  }

  pub fn get_row_document_id(&self, row_id: &RowId) -> Option<String> {
    let row_id = Uuid::parse_str(row_id).ok()?;
    Some(meta_id_from_row_id(&row_id, RowMetaKey::DocumentId))
  }

  /// If the row with given id not exist. It will return an empty row with given id.
  /// An empty [Row] is a row with no cells.
  ///
  pub async fn get_rows_from_row_orders(&self, row_orders: &[RowOrder]) -> Vec<Row> {
    let mut rows = Vec::new();
    for row_order in row_orders {
      let row = if let Some(mutex_row) = self.get_or_init_row(&row_order.id).await {
        mutex_row
          .lock()
          .await
          .get_row()
          .unwrap_or_else(|| Row::empty(row_order.id.clone()))
      } else {
        Row::empty(row_order.id.clone())
      };
      rows.push(row);
    }
    rows
  }

  pub async fn get_cell(&self, row_id: &RowId, field_id: &str) -> Option<Cell> {
    self
      .get_or_init_row(row_id)
      .await?
      .lock()
      .await
      .get_cell(field_id)
  }

  pub async fn delete_row(&self, row_id: &RowId) {
    let row = self.cache.lock().pop(row_id);
    if let Some(row) = row {
      row.lock().await.delete().await;
    }
  }

  pub async fn update_row<F>(&self, row_id: &RowId, f: F)
  where
    F: FnOnce(RowUpdate),
  {
    let row = self.cache.lock().get(row_id).cloned();
    if let Some(row) = row {
      row.lock().await.update::<F>(f);
    }
  }

  pub async fn update_row_meta<F>(&self, row_id: &RowId, f: F)
  where
    F: FnOnce(RowMetaUpdate),
  {
    let row = self.cache.lock().get(row_id).cloned();
    if let Some(row) = row {
      row.lock().await.update_meta::<F>(f);
    }
  }

  /// Get the [DatabaseRow] from the cache. If the row is not in the cache, initialize it.
  async fn get_or_init_row(&self, row_id: &RowId) -> Option<Arc<MutexDatabaseRow>> {
    let collab_db = self.collab_db.upgrade()?;
    let row = self.cache.lock().get(row_id).cloned();
    match row {
      None => {
        let is_exist = collab_db
          .is_exist(self.uid, row_id.as_ref())
          .await
          .unwrap_or(false);
        if !is_exist {
          //
          let (sender, mut rx) = tokio::sync::mpsc::channel(1);
          self.task_controller.add_task(BlockTask::FetchRow {
            uid: self.uid,
            row_id: row_id.clone(),
            seq: self.sequence.fetch_add(1, Ordering::SeqCst),
            sender,
          });

          let weak_notifier = Arc::downgrade(&self.notifier);
          af_spawn(async move {
            while let Some(row_detail) = rx.recv().await {
              if let Some(notifier) = weak_notifier.upgrade() {
                let _ = notifier.send(BlockEvent::DidFetchRow(vec![row_detail]));
              }
            }
          });

          return None;
        }

        let collab = self.collab_for_row(row_id);
        let row = Arc::new(MutexDatabaseRow::new(DatabaseRow::new(
          self.uid,
          row_id.clone(),
          self.collab_db.clone(),
          collab,
          self.row_change_tx.clone(),
        )));
        self.cache.lock().put(row_id.clone(), row.clone());
        Some(row)
      },
      Some(row) => Some(row),
    }
  }

  fn collab_for_row(&self, row_id: &RowId) -> Arc<MutexCollab> {
    let config = CollabPersistenceConfig::new().snapshot_per_update(100);
    let doc_state = CollabDocState::default();
    self.collab_service.build_collab_with_config(
      self.uid,
      row_id,
      CollabType::DatabaseRow,
      self.collab_db.clone(),
      doc_state,
      config,
    )
  }
}
