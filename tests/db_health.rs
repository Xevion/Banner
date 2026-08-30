use assert2::check;
use banner::data::health::ping;
use sqlx::PgPool;

#[sqlx::test]
async fn test_ping_healthy_pool_succeeds(pool: PgPool) {
    check!(ping(&pool).await.is_ok());
}

/// The property that makes readiness worth having: unlike liveness, it fails when the DB is gone.
#[sqlx::test]
async fn test_ping_closed_pool_reports_failure(pool: PgPool) {
    pool.close().await;

    check!(ping(&pool).await.is_err());
}
