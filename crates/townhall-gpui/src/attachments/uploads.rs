use std::sync::Arc;

use townhall_core::model::Attachment;

use super::validate::{is_image, FileMeta};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Uploading,
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingUpload {
    pub key: String,
    pub meta: FileMeta,
    pub bytes: Arc<Vec<u8>>,
    pub is_image: bool,
    pub status: Status,
    pub percent: u8,
    pub attachment: Option<Attachment>,
    pub error: Option<String>,
    pub retryable: bool,
}

#[derive(Debug, Clone)]
pub enum Action {
    Added(Vec<PendingUpload>),
    Progressed {
        key: String,
        percent: u8,
    },
    Finished {
        key: String,
        attachment: Attachment,
    },
    Failed {
        key: String,
        error: String,
        retryable: bool,
    },
    Restarted {
        key: String,
    },
    Removed {
        key: String,
    },
    Cleared,
}

pub fn new_upload(
    key: &str,
    meta: FileMeta,
    bytes: Arc<Vec<u8>>,
    rejection: Option<String>,
) -> PendingUpload {
    PendingUpload {
        key: key.to_string(),
        is_image: is_image(&meta.mime),
        meta,
        bytes,
        status: if rejection.is_none() {
            Status::Uploading
        } else {
            Status::Failed
        },
        percent: 0,
        attachment: None,
        error: rejection,
        retryable: false,
    }
}

fn while_uploading(
    uploads: &mut [PendingUpload],
    key: &str,
    change: impl FnOnce(&mut PendingUpload),
) {
    if let Some(upload) = uploads
        .iter_mut()
        .find(|u| u.key == key && u.status == Status::Uploading)
    {
        change(upload);
    }
}

pub fn reduce(uploads: &mut Vec<PendingUpload>, action: Action) {
    match action {
        Action::Added(added) => uploads.extend(added),
        Action::Progressed { key, percent } => {
            while_uploading(uploads, &key, |u| u.percent = percent)
        }
        Action::Finished { key, attachment } => while_uploading(uploads, &key, |u| {
            u.status = Status::Done;
            u.percent = 100;
            u.attachment = Some(attachment);
        }),
        Action::Failed {
            key,
            error,
            retryable,
        } => while_uploading(uploads, &key, |u| {
            u.status = Status::Failed;
            u.error = Some(error);
            u.retryable = retryable;
        }),
        Action::Restarted { key } => {
            if let Some(u) = uploads
                .iter_mut()
                .find(|u| u.key == key && u.status == Status::Failed)
            {
                u.status = Status::Uploading;
                u.percent = 0;
                u.error = None;
                u.retryable = false;
            }
        }
        Action::Removed { key } => uploads.retain(|u| u.key != key),
        Action::Cleared => uploads.clear(),
    }
}

pub fn is_uploading(uploads: &[PendingUpload]) -> bool {
    uploads.iter().any(|u| u.status == Status::Uploading)
}

pub fn done_attachment_ids(uploads: &[PendingUpload]) -> Vec<String> {
    uploads
        .iter()
        .filter(|u| u.status == Status::Done)
        .filter_map(|u| u.attachment.as_ref().map(|a| a.id.clone()))
        .collect()
}

pub fn latest_error(uploads: &[PendingUpload]) -> Option<String> {
    uploads
        .iter()
        .rev()
        .find(|u| u.status == Status::Failed)
        .and_then(|u| u.error.clone())
}

pub fn can_retry(uploads: &[PendingUpload]) -> bool {
    uploads
        .iter()
        .any(|u| u.status == Status::Failed && u.retryable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use townhall_core::model::AttachmentKind;

    fn meta(name: &str, mime: &str) -> FileMeta {
        FileMeta {
            name: name.into(),
            mime: mime.into(),
            size: 1,
        }
    }

    fn shot() -> Attachment {
        Attachment {
            id: "a1".into(),
            conversation_id: "c1".into(),
            name: "shot.png".into(),
            mime: "image/png".into(),
            size: 1,
            kind: AttachmentKind::Image,
            created_at: "2026-11-12T10:00:00.000Z".into(),
        }
    }

    fn started() -> Vec<PendingUpload> {
        let mut list = Vec::new();
        reduce(
            &mut list,
            Action::Added(vec![
                new_upload("u1", meta("shot.png", "image/png"), Arc::default(), None),
                new_upload("u2", meta("schema.sql", ""), Arc::default(), None),
            ]),
        );
        list
    }

    fn keys(list: &[PendingUpload]) -> Vec<&str> {
        list.iter().map(|u| u.key.as_str()).collect()
    }

    #[test]
    fn an_accepted_file_starts_uploading_at_zero() {
        let u = new_upload("u1", meta("shot.png", "image/png"), Arc::default(), None);
        assert!(u.is_image);
        assert_eq!(u.status, Status::Uploading);
        assert_eq!(u.percent, 0);
        assert_eq!(u.attachment, None);
        assert_eq!(u.error, None);
    }

    #[test]
    fn a_refused_file_fails_for_good() {
        let u = new_upload(
            "u2",
            meta("schema.sql", ""),
            Arc::default(),
            Some("schema.sql is over 20 MB".into()),
        );
        assert!(!u.is_image);
        assert_eq!(u.status, Status::Failed);
        assert_eq!(u.error.as_deref(), Some("schema.sql is over 20 MB"));
        assert!(!u.retryable);
    }

    #[test]
    fn added_uploads_are_appended() {
        let mut list = started();
        reduce(
            &mut list,
            Action::Added(vec![new_upload(
                "u3",
                meta("a.png", "image/png"),
                Arc::default(),
                None,
            )]),
        );
        assert_eq!(keys(&list), ["u1", "u2", "u3"]);
    }

    #[test]
    fn progress_moves_only_a_running_upload() {
        let mut list = started();
        reduce(
            &mut list,
            Action::Progressed {
                key: "u1".into(),
                percent: 60,
            },
        );
        assert_eq!((list[0].percent, list[1].percent), (60, 0));
        reduce(
            &mut list,
            Action::Finished {
                key: "u1".into(),
                attachment: shot(),
            },
        );
        reduce(
            &mut list,
            Action::Progressed {
                key: "u1".into(),
                percent: 10,
            },
        );
        assert_eq!(list[0].percent, 100);
    }

    #[test]
    fn finishing_keeps_the_attachment() {
        let mut list = started();
        reduce(
            &mut list,
            Action::Finished {
                key: "u1".into(),
                attachment: shot(),
            },
        );
        assert_eq!(list[0].status, Status::Done);
        assert_eq!(list[0].percent, 100);
        assert_eq!(list[0].attachment, Some(shot()));
    }

    #[test]
    fn a_failed_upload_keeps_its_reason_and_restarts_clean() {
        let mut list = started();
        reduce(
            &mut list,
            Action::Failed {
                key: "u2".into(),
                error: "cannot reach the daemon".into(),
                retryable: true,
            },
        );
        assert_eq!(list[1].status, Status::Failed);
        assert_eq!(list[1].error.as_deref(), Some("cannot reach the daemon"));
        assert!(list[1].retryable && can_retry(&list));
        reduce(&mut list, Action::Restarted { key: "u2".into() });
        assert_eq!(list[1].status, Status::Uploading);
        assert_eq!(list[1].percent, 0);
        assert_eq!(list[1].error, None);
        assert!(!can_retry(&list));
    }

    #[test]
    fn one_upload_is_removed_or_all_are_cleared() {
        let mut list = started();
        reduce(&mut list, Action::Removed { key: "u1".into() });
        assert_eq!(keys(&list), ["u2"]);
        reduce(&mut list, Action::Removed { key: "nope".into() });
        assert_eq!(keys(&list), ["u2"]);
        reduce(&mut list, Action::Cleared);
        assert!(list.is_empty());
    }

    #[test]
    fn selectors_report_running_done_and_the_latest_failure() {
        let mut list = started();
        assert!(is_uploading(&list));
        assert_eq!(latest_error(&list), None);
        reduce(
            &mut list,
            Action::Finished {
                key: "u1".into(),
                attachment: shot(),
            },
        );
        assert_eq!(done_attachment_ids(&list), ["a1"]);
        reduce(
            &mut list,
            Action::Failed {
                key: "u2".into(),
                error: "two".into(),
                retryable: false,
            },
        );
        assert!(!is_uploading(&list));
        assert_eq!(latest_error(&list).as_deref(), Some("two"));
    }
}
