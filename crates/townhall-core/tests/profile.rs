use tempfile::TempDir;
use townhall_core::events::EventBus;
use townhall_core::model::Settings;
use townhall_core::store::{Store, MAX_PROFILE_CHARS};
use townhall_core::{Error, Home};

fn home(dir: &TempDir) -> Home {
    Home::at(dir.path().join(".townhall"))
}

fn open(dir: &TempDir) -> Store {
    Store::open(&home(dir), EventBus::new()).unwrap()
}

#[tokio::test]
async fn the_profile_is_one_text_that_is_replaced_and_reset() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    assert_eq!(store.profile().await.unwrap(), "");
    let first = store.set_profile("i write short commits").await.unwrap();
    assert_eq!(first, "i write short commits");
    assert_eq!(store.set_profile("tone: dry").await.unwrap(), "tone: dry");
    drop(store);

    let again = open(&dir);
    assert_eq!(
        again.profile().await.unwrap(),
        "tone: dry",
        "it survives a reopen"
    );
    again.delete_profile().await.unwrap();
    assert_eq!(again.profile().await.unwrap(), "");
    again.delete_profile().await.unwrap();
}

#[tokio::test]
async fn the_profile_has_a_size_limit_and_is_not_a_setting() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    let too_long = "x".repeat(MAX_PROFILE_CHARS + 1);
    let err = store.set_profile(&too_long).await.unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
    assert_eq!(store.profile().await.unwrap(), "");
    let fits = "x".repeat(MAX_PROFILE_CHARS);
    store.set_profile(&fits).await.unwrap();
    assert_eq!(store.settings().await.unwrap(), Settings::default());
}
