use crate::helpers::db::test_db;
use banner::data::admin_audits::{AdminAction, AdminAuditFilter, AdminEntity, Target, insert, list};
use banner::data::models::User;
use chrono::Utc;
use serde_json::json;
use strum::VariantArray;

fn actor(discord_id: i64, username: &str) -> User {
    let now = Utc::now();
    User {
        discord_id,
        discord_username: username.to_string(),
        discord_avatar_hash: None,
        is_admin: true,
        created_at: now,
        updated_at: now,
    }
}

#[tokio::test]
async fn test_recorded_entry_round_trips_its_detail() {
    let pool = test_db!().await;
    insert(
        &pool,
        &actor(1, "xevion"),
        AdminAction::InstructorMerge,
        Target::pair(41, 42),
        json!({ "loserId": 42, "namesConfirmed": true, "names": ["Doe, Jane", "Doe, J"] }),
    )
    .await
    .expect("failed to record entry");

    let page = list(&pool, &AdminAuditFilter::default())
        .await
        .expect("failed to list entries");

    assert_eq!(page.total, 1);
    let entry = &page.entries[0];
    assert_eq!(entry.action, "instructor_merge");
    assert_eq!(entry.entity_type, "instructor");
    assert_eq!(entry.entity_id.as_deref(), Some("41"));
    assert_eq!(entry.related_ids, vec!["42".to_string()]);
    assert_eq!(entry.actor_discord_id, 1);
    assert_eq!(entry.actor_username, "xevion");
    assert_eq!(entry.detail.0["names"][1], json!("Doe, J"));
    assert_eq!(entry.detail.0["namesConfirmed"], json!(true));
}

/// A sweep over every record of a kind has no single target.
#[tokio::test]
async fn test_untargeted_action_records_a_null_entity_id() {
    let pool = test_db!().await;
    insert(
        &pool,
        &actor(1, "xevion"),
        AdminAction::InstructorMergeAll,
        Target::all(),
        json!({ "merged": 3, "skipped": 1 }),
    )
    .await
    .expect("failed to record entry");

    let page = list(&pool, &AdminAuditFilter::default()).await.unwrap();

    assert_eq!(page.entries[0].entity_id, None);
    assert!(page.entries[0].related_ids.is_empty());
}

#[tokio::test]
async fn test_every_action_records_and_lists_under_its_own_name() {
    let pool = test_db!().await;
    for action in AdminAction::VARIANTS {
        insert(&pool, &actor(1, "xevion"), *action, Target::id(7), json!({}))
            .await
            .expect("failed to record entry");
    }

    for action in AdminAction::VARIANTS {
        let filter = AdminAuditFilter {
            action: Some(*action),
            ..AdminAuditFilter::default()
        };
        let page = list(&pool, &filter).await.unwrap();

        assert_eq!(page.total, 1, "expected one entry for {action:?}");
        assert_eq!(page.entries[0].entity_type, action.entity().as_ref());
    }
}

#[tokio::test]
async fn test_listing_filters_by_actor() {
    let pool = test_db!().await;
    insert(
        &pool,
        &actor(1, "first"),
        AdminAction::TermEnable,
        Target::id("202610"),
        json!({}),
    )
    .await
    .unwrap();
    insert(
        &pool,
        &actor(2, "second"),
        AdminAction::TermDisable,
        Target::id("202620"),
        json!({}),
    )
    .await
    .unwrap();

    let filter = AdminAuditFilter {
        actor_discord_id: Some(2),
        ..AdminAuditFilter::default()
    };
    let page = list(&pool, &filter).await.unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.entries[0].actor_username, "second");
}

#[tokio::test]
async fn test_listing_filters_by_entity_type() {
    let pool = test_db!().await;
    insert(
        &pool,
        &actor(1, "xevion"),
        AdminAction::UserSetAdmin,
        Target::id(99),
        json!({ "isAdmin": true }),
    )
    .await
    .unwrap();
    insert(
        &pool,
        &actor(1, "xevion"),
        AdminAction::TermEnable,
        Target::id("202610"),
        json!({}),
    )
    .await
    .unwrap();

    let filter = AdminAuditFilter {
        entity_type: Some(AdminEntity::Term),
        ..AdminAuditFilter::default()
    };
    let page = list(&pool, &filter).await.unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.entries[0].entity_id.as_deref(), Some("202610"));
}

/// Asking what happened to one instructor must find merges it lost as well as won.
#[tokio::test]
async fn test_listing_by_entity_id_matches_the_other_side_of_a_pair() {
    let pool = test_db!().await;
    insert(
        &pool,
        &actor(1, "xevion"),
        AdminAction::InstructorMerge,
        Target::pair(41, 42),
        json!({}),
    )
    .await
    .unwrap();
    insert(
        &pool,
        &actor(1, "xevion"),
        AdminAction::InstructorDismiss,
        Target::pair(50, 51),
        json!({}),
    )
    .await
    .unwrap();

    let filter = AdminAuditFilter {
        entity_id: Some("42".to_string()),
        ..AdminAuditFilter::default()
    };
    let page = list(&pool, &filter).await.unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.entries[0].action, "instructor_merge");
}

#[tokio::test]
async fn test_listing_filters_by_time_range() {
    let pool = test_db!().await;
    let first = insert(
        &pool,
        &actor(1, "xevion"),
        AdminAction::TermEnable,
        Target::id("202610"),
        json!({}),
    )
    .await
    .unwrap();
    let second = insert(
        &pool,
        &actor(1, "xevion"),
        AdminAction::TermDisable,
        Target::id("202620"),
        json!({}),
    )
    .await
    .unwrap();

    let since = AdminAuditFilter {
        since: Some(second.created_at),
        ..AdminAuditFilter::default()
    };
    let page = list(&pool, &since).await.unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.entries[0].id, second.id);

    let until = AdminAuditFilter {
        until: Some(second.created_at),
        ..AdminAuditFilter::default()
    };
    let page = list(&pool, &until).await.unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.entries[0].id, first.id);
}

#[tokio::test]
async fn test_listing_paginates_newest_first() {
    let pool = test_db!().await;
    for code in ["202610", "202620", "202630"] {
        insert(
            &pool,
            &actor(1, "xevion"),
            AdminAction::TermEnable,
            Target::id(code),
            json!({}),
        )
        .await
        .unwrap();
    }

    let filter = AdminAuditFilter {
        per_page: 2,
        ..AdminAuditFilter::default()
    };
    let first = list(&pool, &filter).await.unwrap();

    assert_eq!(first.total, 3);
    assert_eq!(first.entries.len(), 2);
    assert_eq!(first.entries[0].entity_id.as_deref(), Some("202630"));

    let second = list(
        &pool,
        &AdminAuditFilter {
            page: 2,
            per_page: 2,
            ..AdminAuditFilter::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(second.entries.len(), 1);
    assert_eq!(second.entries[0].entity_id.as_deref(), Some("202610"));
}

/// The action has already committed by the time it is audited, so a broken
/// audit write must leave it standing.
#[tokio::test]
async fn test_failed_audit_write_leaves_the_action_intact() {
    let pool = test_db!().await;
    let first = crate::helpers::insert_instructor(&pool, "Doe, Jane", None).await;
    let second = crate::helpers::insert_instructor(&pool, "Doe, J", None).await;

    sqlx::query("DROP TABLE admin_audits")
        .execute(&pool)
        .await
        .expect("failed to drop the audit table");

    banner::data::instructor_merge::dismiss_pair(&pool, first, second, None)
        .await
        .expect("dismiss should succeed");

    banner::web::admin::action_log::record(
        &pool,
        &actor(1, "xevion"),
        AdminAction::InstructorDismiss,
        Target::pair(first, second),
        json!({}),
    )
    .await;

    let (dismissals,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM instructor_dismissals")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(dismissals, 1);
}
