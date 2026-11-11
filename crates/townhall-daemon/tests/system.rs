use axum::http::StatusCode;
use townhall_daemon::router;

mod common;
use common::{call, Env};

#[tokio::test]
async fn system_stats_are_served_as_camel_case_json() {
    let env = Env::new("startup").await;
    let app = router(env.state());

    let (status, v) = call(&app, "GET", "/api/system", None).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["success"], true);
    let d = &v["data"];

    let cpu = d["cpuPercent"].as_f64().expect("cpuPercent is a number");
    assert!((0.0..=100.0).contains(&cpu), "{d}");

    let mem_total = d["memoryTotalBytes"].as_u64().expect("memoryTotalBytes");
    let mem_used = d["memoryUsedBytes"].as_u64().expect("memoryUsedBytes");
    assert!(
        mem_total > 0 && mem_used > 0 && mem_used <= mem_total,
        "{d}"
    );

    let disk_total = d["diskTotalBytes"].as_u64().expect("diskTotalBytes");
    let disk_used = d["diskUsedBytes"].as_u64().expect("diskUsedBytes");
    assert!(disk_total > 0 && disk_used <= disk_total, "{d}");
}

#[tokio::test]
async fn system_stats_reject_other_methods() {
    let env = Env::new("startup").await;
    let app = router(env.state());
    let (status, _) = call(&app, "POST", "/api/system", None).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}
