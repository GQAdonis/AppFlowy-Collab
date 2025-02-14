use std::future::Future;
use std::time::Duration;

use collab_folder::{SectionChange, SectionChangeReceiver, TrashSectionChange, UserId};

use crate::util::{create_folder_with_workspace, make_test_view};

#[tokio::test]
async fn create_trash_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1").await;
  let view_1 = make_test_view("v1", "w1", vec![]);
  let view_2 = make_test_view("v2", "w1", vec![]);
  let view_3 = make_test_view("v3", "w1", vec![]);
  folder_test.insert_view(view_1, Some(0));
  folder_test.insert_view(view_2, Some(0));
  folder_test.insert_view(view_3, Some(0));
  folder_test.add_trash(vec!["v1".to_string(), "v2".to_string(), "v3".to_string()]);

  let trash = folder_test.get_all_trash();
  assert_eq!(trash.len(), 3);
  assert_eq!(trash[0].id, "v1");
  assert_eq!(trash[1].id, "v2");
  assert_eq!(trash[2].id, "v3");
}

#[tokio::test]
async fn delete_trash_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1").await;
  let view_1 = make_test_view("v1", "w1", vec![]);
  let view_2 = make_test_view("v2", "w1", vec![]);
  folder_test.insert_view(view_1, Some(0));
  folder_test.insert_view(view_2, Some(0));

  folder_test.add_trash(vec!["v1".to_string(), "v2".to_string()]);

  let trash = folder_test.get_all_trash();
  assert_eq!(trash[0].id, "v1");
  assert_eq!(trash[1].id, "v2");

  folder_test.delete_trash(vec!["v1".to_string()]);
  let trash = folder_test.get_all_trash();
  assert_eq!(trash[0].id, "v2");
}

#[tokio::test]
async fn create_trash_callback_test() {
  let uid = UserId::from(1);
  let mut folder_test = create_folder_with_workspace(uid.clone(), "w1").await;
  let section_rx = folder_test.section_rx.take().unwrap();
  tokio::spawn(async move {
    folder_test.add_trash(vec!["1".to_string(), "2".to_string()]);
  });

  timeout(poll_tx(section_rx, |change| match change {
    SectionChange::Trash(change) => match change {
      TrashSectionChange::TrashItemAdded { ids } => {
        assert_eq!(ids, vec!["1", "2"]);
      },
      TrashSectionChange::TrashItemRemoved { .. } => {},
    },
  }))
  .await;
}

#[tokio::test]
async fn delete_trash_callback_test() {
  let uid = UserId::from(1);
  let mut folder_test = create_folder_with_workspace(uid.clone(), "w1").await;
  let trash_rx = folder_test.section_rx.take().unwrap();
  tokio::spawn(async move {
    folder_test.add_trash(vec!["1".to_string(), "2".to_string()]);
    folder_test.delete_trash(vec!["1".to_string(), "2".to_string()]);
  });

  timeout(poll_tx(trash_rx, |change| match change {
    SectionChange::Trash(change) => match change {
      TrashSectionChange::TrashItemAdded { ids } => {
        assert_eq!(ids, vec!["1", "2"]);
      },
      TrashSectionChange::TrashItemRemoved { ids } => {
        assert_eq!(ids, vec!["1", "2"]);
      },
    },
  }))
  .await;
}

async fn poll_tx(mut rx: SectionChangeReceiver, callback: impl Fn(SectionChange)) {
  while let Ok(change) = rx.recv().await {
    callback(change)
  }
}

async fn timeout<F: Future>(f: F) {
  tokio::time::timeout(Duration::from_secs(2), f)
    .await
    .unwrap();
}
