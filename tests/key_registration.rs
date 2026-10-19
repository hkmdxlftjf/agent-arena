//! A registration success must mean this agent can actually select the key.

mod common;

use arena::app::App;
use arena::config::Config;
use arena::crypto::ServerKey;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test]
async fn shared_local_key_cannot_report_registration_success_for_another_agent(pool: PgPool) {
    let dir = common::fresh_home("key-owner");
    let app = App::boot(
        pool,
        ServerKey::load_or_create(&dir).unwrap(),
        Config::for_test(dir.clone()),
    )
    .await
    .unwrap();
    let (owner, _) = app
        .register("keys@example.com", "password123", "key_owner")
        .await
        .unwrap();
    let key = common::keygen(&dir);
    let mut agents = Vec::new();
    for name in ["first", "second"] {
        let created = app
            .create_agent(&owner, name, None, None, false)
            .await
            .unwrap();
        let id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
        let issued = app.issue_token(&owner, id).await.unwrap();
        agents.push(
            app.authenticate(issued["token"].as_str().unwrap())
                .await
                .unwrap(),
        );
    }
    let challenge = app.key_challenge(&agents[0]).await.unwrap();
    let envelope = common::sign(&dir, &challenge["signing_request"], None, None);
    app.register_key(&agents[0], &envelope, None).await.unwrap();
    let again = app.key_challenge(&agents[0]).await.unwrap();
    let envelope = common::sign(&dir, &again["signing_request"], None, None);
    assert_eq!(
        app.register_key(&agents[0], &envelope, None).await.unwrap()["idempotent"],
        true
    );

    // When the same local signer attempts registration for another agent.
    let challenge = app.key_challenge(&agents[1]).await.unwrap();
    let envelope = common::sign(&dir, &challenge["signing_request"], None, None);
    let error = app
        .register_key(&agents[1], &envelope, None)
        .await
        .unwrap_err();

    // Then it is a conflict, not a false success followed by unknown_key.
    assert_eq!(error.status, 409);
    assert_eq!(error.code, "key_bound_to_other_agent");
    assert!(app.whoami(&agents[1]).await.unwrap()["keys"]
        .as_array()
        .unwrap()
        .is_empty());
    let room = app
        .create_room(&agents[0], &json!({"game":"gomoku", "kid":key["kid"]}))
        .await
        .unwrap();
    app.leave_room(
        agents[0].agent_id,
        Uuid::parse_str(room["match_id"].as_str().unwrap()).unwrap(),
    )
    .await
    .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[sqlx::test]
async fn inactive_key_cannot_report_successful_registration(pool: PgPool) {
    let dir = common::fresh_home("key-inactive");
    let app = App::boot(
        pool,
        ServerKey::load_or_create(&dir).unwrap(),
        Config::for_test(dir.clone()),
    )
    .await
    .unwrap();
    let (owner, _) = app
        .register("inactive@example.com", "password123", "inactive_owner")
        .await
        .unwrap();
    let created = app
        .create_agent(&owner, "inactive", None, None, false)
        .await
        .unwrap();
    let issued = app
        .issue_token(
            &owner,
            Uuid::parse_str(created["id"].as_str().unwrap()).unwrap(),
        )
        .await
        .unwrap();
    let agent = app
        .authenticate(issued["token"].as_str().unwrap())
        .await
        .unwrap();
    let key = common::keygen(&dir);
    let challenge = app.key_challenge(&agent).await.unwrap();
    let envelope = common::sign(&dir, &challenge["signing_request"], None, None);
    app.register_key(&agent, &envelope, None).await.unwrap();

    // Given an existing key that is no longer eligible for new seats.
    for status in ["revoked", "rotated"] {
        sqlx::query("UPDATE agent_keys SET status = $1 WHERE kid = $2")
            .bind(status)
            .bind(key["kid"].as_str().unwrap())
            .execute(app.pool())
            .await
            .unwrap();
        let challenge = app.key_challenge(&agent).await.unwrap();
        let envelope = common::sign(&dir, &challenge["signing_request"], None, None);

        // When registration is retried, then it must not claim activation.
        let error = app.register_key(&agent, &envelope, None).await.unwrap_err();
        assert_eq!(error.status, 409);
        assert_eq!(error.code, "key_inactive");
    }
    std::fs::remove_dir_all(dir).unwrap();
}
