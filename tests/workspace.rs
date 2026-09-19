use s1code::{domain::*, session::Store, tools::Workspace};
use std::fs;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn search_respects_nested_ignores_and_refreshes_between_actions() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    for name in ["visible.txt", "private.txt", "excluded.txt"] {
        fs::write(root.path().join("src").join(name), "needle").unwrap();
    }
    fs::write(root.path().join("src/.gitignore"), "private.txt\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        root.path().join("src/private.txt"),
        root.path().join("link.txt"),
    )
    .unwrap();
    let workspace = Workspace::new(root.path(), vec!["src/excluded.txt".into()]).unwrap();
    let (store, _) = Store::create(
        home.path(),
        root.path(),
        "search".into(),
        RunConfig::default(),
    )
    .unwrap();
    let search = Action::Search {
        query: "needle".into(),
    };
    let cancel = CancellationToken::new();
    let result = workspace.execute(&search, &store, &cancel).await.unwrap();
    assert_eq!(result.text.trim(), "src/visible.txt:1:needle");
    fs::write(
        root.path().join("src/.gitignore"),
        "private.txt\nvisible.txt\n",
    )
    .unwrap();
    let result = workspace.execute(&search, &store, &cancel).await.unwrap();
    assert!(
        result.text.is_empty(),
        "discovery must not be cached across actions"
    );
}

#[test]
fn revision_rejects_oversized_files_and_tracks_content_not_just_length() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("file");
    fs::write(&path, "before").unwrap();
    let workspace = Workspace::new(root.path(), vec![]).unwrap();
    let before = workspace.revision().unwrap();
    fs::write(&path, "after!").unwrap();
    assert_ne!(before, workspace.revision().unwrap());
    fs::File::create(&path)
        .unwrap()
        .set_len(128 * 1024 * 1024 + 1)
        .unwrap();
    assert!(
        workspace
            .revision()
            .unwrap_err()
            .to_string()
            .contains("128 MiB")
    );
}

#[test]
#[cfg(unix)]
fn revision_rejects_symlinked_ignore_configuration() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join(".gitignore")).unwrap();
    let workspace = Workspace::new(root.path(), vec![]).unwrap();
    assert!(workspace.revision().is_err());
}
