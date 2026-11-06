use serde_json::{json, Value};

mod common;
mod mcp_support;
use common::unique;
use mcp_support::{task, Mcp};

const TESTER: &str = "name = \"tester\"\nadapter = \"claude\"\n";
const REVIEWER: &str = "name = \"reviewer\"\nadapter = \"claude\"\n";
const SHIPPER: &str =
    "name = \"shipper\"\nadapter = \"claude\"\nrequires = [\"tester\", \"reviewer\"]\n";

async fn with_types(m: &Mcp) {
    for source in [TESTER, REVIEWER, SHIPPER] {
        m.store().save_agent_type(source).await.unwrap();
    }
    m.note_repo("shop").await;
}

fn typed(name: &str, agent: &str, after: &[&str]) -> Value {
    let mut args = task(name);
    args["agent"] = json!(agent);
    args["after"] = json!(after);
    args
}

#[tokio::test]
async fn a_gated_task_without_its_required_tasks_is_refused_by_name() {
    let m = Mcp::new().await;
    with_types(&m).await;
    let none = m
        .refused("create_task", typed(&unique("ship"), "shipper", &[]))
        .await;
    assert!(
        none.contains("`tester`") && none.contains("`reviewer`"),
        "{none}"
    );

    let tested = unique("test");
    m.ok("create_task", typed(&tested, "tester", &[])).await;
    let half = m
        .refused("create_task", typed(&unique("ship"), "shipper", &[&tested]))
        .await;
    assert!(
        half.contains("`reviewer`") && !half.contains("`tester`"),
        "{half}"
    );
    assert_eq!(m.store().all_tasks().await.unwrap().len(), 1);
}

#[tokio::test]
async fn required_tasks_count_anywhere_in_the_after_chain() {
    let m = Mcp::new().await;
    with_types(&m).await;
    let (tested, reviewed, other) = (unique("test"), unique("review"), unique("other"));
    m.ok("create_task", typed(&tested, "tester", &[])).await;
    m.ok("create_task", typed(&reviewed, "REVIEWER", &[&tested]))
        .await;
    m.ok("create_task", typed(&other, "builder", &[&reviewed]))
        .await;
    let ship = unique("ship");
    let created = m
        .ok("create_task", typed(&ship, "shipper", &[&other]))
        .await;
    assert_eq!(created["agent"], "shipper");
}

#[tokio::test]
async fn a_type_without_requires_needs_no_chain() {
    let m = Mcp::new().await;
    with_types(&m).await;
    let created = m
        .ok("create_task", typed(&unique("plain"), "builder", &[]))
        .await;
    assert_eq!(created["state"], "queued");
}
