use assert2::check;
use banner::data::health::ping;

use crate::helpers::db::test_db;

#[tokio::test]
async fn test_ping_healthy_pool_succeeds() {
    let pool = test_db!().await;
    check!(ping(&pool).await.is_ok());
}

/// The property that makes readiness worth having: unlike liveness, it fails when the DB is gone.
#[tokio::test]
async fn test_ping_closed_pool_reports_failure() {
    let pool = test_db!().await;
    pool.close().await;

    check!(ping(&pool).await.is_err());
}
