//! Interactive demo mode that showcases all vibetracer features with synthetic data.

use crate::claude_log::{ConversationTurn, ToolCall};
use crate::config::Config;
use crate::event::{EditEvent, EditKind};
use crate::tui::alerts::AlertEvaluator;
use crate::tui::app::{Mode, PlaybackState, ToastStyle};
use crate::tui::{App, RunOptions};
use std::path::PathBuf;

/// Run the interactive demo.
pub fn run_demo(project_path: PathBuf, config: Config) -> anyhow::Result<()> {
    let mut app = App::new();

    // Session starts 15 minutes ago
    let now_ms = chrono::Utc::now().timestamp_millis();
    let session_start_ms = now_ms - 900_000;
    app.session_start = session_start_ms / 1000;

    // ── Generate synthetic edits ─────────────────────────────────────────────
    let edits = generate_edits(session_start_ms);
    for edit in edits {
        app.push_edit(edit);
    }

    // ── Set playhead to middle of session ────────────────────────────────────
    app.playhead = 25;
    app.playback = PlaybackState::Paused;
    app.connected = true;
    app.dashboard_visible = true;
    app.mode = Mode::Normal;

    // ── Pre-populate conversation turns ──────────────────────────────────────
    app.conversation_turns = generate_conversation_turns(session_start_ms);
    app.token_stats = crate::claude_log::compute_stats(&app.conversation_turns);

    // ── Pre-populate dashboard sparklines ─────────────────────────────────────
    let velocity_data = [0.0, 1.0, 3.0, 8.0, 12.0, 8.0, 3.0, 1.0, 2.0, 6.0, 10.0, 14.0, 8.0, 4.0, 2.0, 5.0, 9.0, 6.0, 3.0, 1.0];
    let token_data = [0.0, 0.0, 4.0, 8.2, 12.4, 8.2, 3.0, 0.0, 0.0, 6.0, 12.4, 9.8, 4.0, 0.0, 2.0, 7.6, 7.6, 4.0, 2.0, 0.0];
    let cost_data = [0.0, 0.0, 0.02, 0.04, 0.06, 0.04, 0.01, 0.0, 0.0, 0.03, 0.06, 0.05, 0.02, 0.0, 0.01, 0.04, 0.04, 0.02, 0.01, 0.0];

    for &v in &velocity_data {
        app.dashboard_state.velocity_sparkline.push(v);
    }
    for &v in &token_data {
        app.dashboard_state.token_rate.push(v);
    }
    for &v in &cost_data {
        app.dashboard_state.cost_rate.push(v);
    }

    // ── Pre-populate analysis state ──────────────────────────────────────────
    app.sentinel_violations = vec![
        crate::analysis::sentinels::SentinelViolation {
            rule_name: "feature_count".to_string(),
            description: "model input size mismatch".to_string(),
            value_a: "12".to_string(),
            value_b: "14".to_string(),
            assertion: "eq".to_string(),
        },
        crate::analysis::sentinels::SentinelViolation {
            rule_name: "api_version".to_string(),
            description: "version inconsistent across files".to_string(),
            value_a: "v2".to_string(),
            value_b: "v3".to_string(),
            assertion: "eq".to_string(),
        },
    ];

    app.watchdog_alerts = vec![crate::analysis::watchdog::WatchdogAlert {
        constant_pattern: "MAX_RETRIES".to_string(),
        expected: "3".to_string(),
        actual: "5".to_string(),
        severity: "warning".to_string(),
        file: "src/config.rs".to_string(),
    }];

    app.blast_radius_status = Some((
        "src/auth.rs".to_string(),
        crate::analysis::blast_radius::DependencyStatus {
            updated: vec![
                "src/middleware.rs".to_string(),
                "src/config.rs".to_string(),
                "tests/auth_test.rs".to_string(),
                "src/api/login.rs".to_string(),
                "README.md".to_string(),
            ],
            stale: vec![
                "src/session.rs".to_string(),
                "src/api/refresh.rs".to_string(),
                "src/cache.rs".to_string(),
            ],
            untouched: vec![
                "src/main.rs".to_string(),
                "src/lib.rs".to_string(),
                "src/db.rs".to_string(),
            ],
        },
    ));

    // ── Pre-populate bookmarks ───────────────────────────────────────────────
    app.bookmark_manager.add("session start".to_string(), 0);
    app.bookmark_manager.add("JWT migration complete".to_string(), 18);
    app.bookmark_manager.add("rate limiting added".to_string(), 26);
    app.bookmark_manager.add("tests updated".to_string(), 32);

    // ── Initialize alert evaluator ───────────────────────────────────────────
    if !config.alerts.is_empty() {
        app.alert_evaluator = AlertEvaluator::new(config.alerts.clone());
    }

    // ── Pre-populate synthetic file contents for preview ────────────────────
    populate_synthetic_content(&mut app);

    // ── Show welcome toast ───────────────────────────────────────────────────
    app.show_toast(
        "demo mode -- explore with arrows, t/i/:/? for modes".to_string(),
        ToastStyle::Info,
    );

    // Run TUI with pre-populated state
    let options = RunOptions {
        initial_app: Some(app),
        no_daemon: true,
    };
    crate::tui::run_tui_with_options(project_path, config, options)?;

    Ok(())
}

/// Generate ~58 realistic synthetic edits simulating a JWT auth migration.
fn generate_edits(session_start_ms: i64) -> Vec<EditEvent> {
    let mut edits = Vec::new();
    let mut id: u64 = 0;
    let mut hash_counter: u64 = 0;

    let mut next_hash = || -> String {
        hash_counter += 1;
        format!("sha256_{:04}", hash_counter)
    };

    let mut add_edit = |edits: &mut Vec<EditEvent>,
                        id: &mut u64,
                        offset_ms: i64,
                        file: &str,
                        kind: EditKind,
                        patch: &str,
                        added: u32,
                        removed: u32,
                        agent: &str,
                        op_id: &str,
                        op_intent: &str,
                        tool: &str| {
        *id += 1;
        let before = next_hash();
        let after = next_hash();
        edits.push(EditEvent {
            id: *id,
            ts: session_start_ms + offset_ms,
            file: file.to_string(),
            kind,
            patch: patch.to_string(),
            before_hash: Some(before),
            after_hash: after,
            intent: Some(op_intent.to_string()),
            tool: Some("claude".to_string()),
            lines_added: added,
            lines_removed: removed,
            agent_id: Some(agent.to_string()),
            agent_label: Some(agent.to_string()),
            operation_id: Some(op_id.to_string()),
            operation_intent: Some(op_intent.to_string()),
            tool_name: Some(tool.to_string()),
            restore_id: None,
        });
    };

    // ── Op 1: Understand auth system (3 edits, 0-60s) ───────────────────────
    add_edit(&mut edits, &mut id, 5_000, "src/auth.rs", EditKind::Modify,
        "@@ -1,5 +1,6 @@\n use std::collections::HashMap;\n+use std::time::Duration;\n \n pub struct AuthConfig {\n     pub session_ttl: u64,\n     pub max_retries: u32,",
        1, 0, "claude-1", "op-1", "Read and understand auth system", "Edit");

    add_edit(&mut edits, &mut id, 12_000, "src/middleware.rs", EditKind::Modify,
        "@@ -15,3 +15,5 @@\n pub fn auth_middleware(req: &Request) -> Result<(), AuthError> {\n     let token = req.header(\"Authorization\");\n+    // TODO: migrate to JWT validation\n+    // Current: session token lookup\n     validate_session(token)",
        2, 0, "claude-1", "op-1", "Read and understand auth system", "Edit");

    add_edit(&mut edits, &mut id, 20_000, "src/config.rs", EditKind::Modify,
        "@@ -8,2 +8,4 @@\n pub struct Config {\n     pub db_url: String,\n+    pub jwt_secret: Option<String>,\n+    pub jwt_expiry_secs: u64,",
        2, 0, "claude-1", "op-1", "Read and understand auth system", "Edit");

    // ── Op 2: Replace session tokens with JWT (15 edits, 60-360s) ────────────
    let op2 = "Replace session tokens with JWT";

    add_edit(&mut edits, &mut id, 65_000, "src/auth.rs", EditKind::Modify,
        "@@ -10,8 +10,14 @@\n-pub fn validate_session(token: &str) -> Result<User, AuthError> {\n-    let session = SESSION_STORE.get(token)?;\n-    if session.is_expired() {\n-        return Err(AuthError::Expired);\n-    }\n-    Ok(session.user.clone())\n+pub fn validate_jwt(token: &str, secret: &[u8]) -> Result<Claims, AuthError> {\n+    let key = DecodingKey::from_secret(secret);\n+    let validation = Validation::new(Algorithm::HS256);\n+    let token_data = decode::<Claims>(token, &key, &validation)\n+        .map_err(|e| match e.kind() {\n+            ErrorKind::ExpiredSignature => AuthError::Expired,\n+            ErrorKind::InvalidToken => AuthError::InvalidToken,\n+            _ => AuthError::Internal(e.to_string()),\n+        })?;\n+    Ok(token_data.claims)\n }",
        10, 6, "claude-1", "op-2", op2, "Edit");

    add_edit(&mut edits, &mut id, 80_000, "src/auth.rs", EditKind::Modify,
        "@@ -25,4 +31,12 @@\n+#[derive(Debug, Serialize, Deserialize)]\n+pub struct Claims {\n+    pub sub: String,\n+    pub exp: usize,\n+    pub iat: usize,\n+    pub role: String,\n+}\n+",
        8, 0, "claude-1", "op-2", op2, "Edit");

    add_edit(&mut edits, &mut id, 95_000, "src/auth.rs", EditKind::Modify,
        "@@ -45,5 +53,15 @@\n-pub fn create_session(user: &User) -> String {\n-    let token = generate_random_token();\n-    SESSION_STORE.insert(token.clone(), Session::new(user));\n-    token\n+pub fn create_jwt(user: &User, secret: &[u8]) -> Result<String, AuthError> {\n+    let now = chrono::Utc::now();\n+    let claims = Claims {\n+        sub: user.id.to_string(),\n+        exp: (now + chrono::Duration::hours(24)).timestamp() as usize,\n+        iat: now.timestamp() as usize,\n+        role: user.role.to_string(),\n+    };\n+    let key = EncodingKey::from_secret(secret);\n+    encode(&Header::default(), &claims, &key)\n+        .map_err(|e| AuthError::Internal(e.to_string()))\n }",
        11, 4, "claude-1", "op-2", op2, "Edit");

    add_edit(&mut edits, &mut id, 110_000, "src/middleware.rs", EditKind::Modify,
        "@@ -15,8 +15,16 @@\n pub fn auth_middleware(req: &Request) -> Result<(), AuthError> {\n-    let token = req.header(\"Authorization\");\n-    // TODO: migrate to JWT validation\n-    // Current: session token lookup\n-    validate_session(token)\n+    let auth_header = req.header(\"Authorization\")\n+        .ok_or(AuthError::MissingToken)?;\n+    let token = auth_header\n+        .strip_prefix(\"Bearer \")\n+        .ok_or(AuthError::InvalidFormat)?;\n+    let secret = req.state().config.jwt_secret.as_bytes();\n+    let claims = validate_jwt(token, secret)?;\n+    req.set_extension(claims);\n+    Ok(())\n }",
        9, 4, "claude-1", "op-2", op2, "Edit");

    add_edit(&mut edits, &mut id, 130_000, "src/middleware.rs", EditKind::Modify,
        "@@ -32,2 +40,8 @@\n+pub fn extract_claims(req: &Request) -> Result<&Claims, AuthError> {\n+    req.extensions()\n+        .get::<Claims>()\n+        .ok_or(AuthError::NotAuthenticated)\n+}\n+",
        6, 0, "claude-1", "op-2", op2, "Edit");

    add_edit(&mut edits, &mut id, 148_000, "src/config.rs", EditKind::Modify,
        "@@ -12,3 +12,6 @@\n-    pub jwt_secret: Option<String>,\n+    pub jwt_secret: String,\n+    pub jwt_refresh_secret: String,\n     pub jwt_expiry_secs: u64,\n+    pub jwt_refresh_expiry_secs: u64,",
        3, 1, "claude-1", "op-2", op2, "Edit");

    add_edit(&mut edits, &mut id, 165_000, "src/auth.rs", EditKind::Modify,
        "@@ -1,3 +1,5 @@\n use std::collections::HashMap;\n use std::time::Duration;\n+use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation, Algorithm};\n+use jsonwebtoken::errors::ErrorKind;\n use serde::{Serialize, Deserialize};",
        2, 0, "claude-1", "op-2", op2, "Edit");

    add_edit(&mut edits, &mut id, 180_000, "src/api/login.rs", EditKind::Modify,
        "@@ -8,6 +8,12 @@\n pub async fn login(req: LoginRequest) -> Result<LoginResponse, ApiError> {\n     let user = authenticate(&req.username, &req.password).await?;\n-    let token = create_session(&user);\n-    Ok(LoginResponse { token })\n+    let config = get_config();\n+    let access_token = create_jwt(&user, config.jwt_secret.as_bytes())?;\n+    let refresh_token = create_refresh_token(&user, config.jwt_refresh_secret.as_bytes())?;\n+    Ok(LoginResponse {\n+        access_token,\n+        refresh_token,\n+        expires_in: config.jwt_expiry_secs,\n+    })\n }",
        8, 2, "claude-1", "op-2", op2, "Edit");

    for i in 0..6 {
        add_edit(&mut edits, &mut id, 200_000 + i * 15_000, "src/auth.rs", EditKind::Modify,
            &format!("@@ -{},{} +{},{} @@\n-    // old auth line {}\n+    // new JWT auth line {}", 60+i*5, 2, 60+i*5, 2, i, i),
            1, 1, "claude-1", "op-2", op2, "Edit");
    }

    // ── Op 3: Add rate limiting (8 edits, 360-540s) ──────────────────────────
    let op3 = "Add rate limiting";

    add_edit(&mut edits, &mut id, 365_000, "src/middleware.rs", EditKind::Modify,
        "@@ -1,3 +1,5 @@\n use std::collections::HashMap;\n+use std::sync::Arc;\n+use tokio::sync::RwLock;\n use crate::auth::Claims;",
        2, 0, "claude-2", "op-3", op3, "Edit");

    add_edit(&mut edits, &mut id, 380_000, "src/middleware.rs", EditKind::Modify,
        "@@ -45,0 +47,18 @@\n+pub struct RateLimiter {\n+    limits: Arc<RwLock<HashMap<String, (u64, std::time::Instant)>>>,\n+    max_requests: u64,\n+    window: Duration,\n+}\n+\n+impl RateLimiter {\n+    pub fn new(max_requests: u64, window: Duration) -> Self {\n+        Self {\n+            limits: Arc::new(RwLock::new(HashMap::new())),\n+            max_requests,\n+            window,\n+        }\n+    }\n+\n+    pub async fn check(&self, ip: &str) -> Result<(), AuthError> {\n+        let mut limits = self.limits.write().await;\n+        // ... rate limit logic",
        18, 0, "claude-2", "op-3", op3, "Edit");

    add_edit(&mut edits, &mut id, 400_000, "src/config.rs", EditKind::Modify,
        "@@ -18,0 +18,4 @@\n+    pub rate_limit_requests: u64,\n+    pub rate_limit_window_secs: u64,\n+    pub rate_limit_burst: u64,\n+    pub rate_limit_enabled: bool,",
        4, 0, "claude-2", "op-3", op3, "Edit");

    add_edit(&mut edits, &mut id, 420_000, "src/api/login.rs", EditKind::Modify,
        "@@ -6,2 +6,8 @@\n+pub async fn rate_limited_login(req: LoginRequest, limiter: &RateLimiter) -> Result<LoginResponse, ApiError> {\n+    let ip = req.remote_addr();\n+    limiter.check(ip).await.map_err(|_| ApiError::RateLimited)?;\n+    login(req).await\n+}\n+",
        6, 0, "claude-2", "op-3", op3, "Edit");

    for i in 0..4 {
        add_edit(&mut edits, &mut id, 440_000 + i * 20_000, "src/middleware.rs", EditKind::Modify,
            &format!("@@ -{},{} +{},{} @@\n+    // rate limit check {}", 65+i*3, 1, 65+i*3, 2, i),
            1, 0, "claude-2", "op-3", op3, "Edit");
    }

    // ── Op 4: Update tests (6 edits, 540-660s) ──────────────────────────────
    let op4 = "Update tests for new auth";

    add_edit(&mut edits, &mut id, 545_000, "tests/auth_test.rs", EditKind::Modify,
        "@@ -1,8 +1,12 @@\n-use crate::auth::validate_session;\n+use crate::auth::{validate_jwt, create_jwt, Claims};\n+use jsonwebtoken::{EncodingKey, DecodingKey};\n \n #[test]\n-fn test_session_validation() {\n-    let token = create_test_session();\n-    let result = validate_session(&token);\n-    assert!(result.is_ok());\n+fn test_jwt_validation() {\n+    let secret = b\"test-secret-key\";\n+    let user = create_test_user();\n+    let token = create_jwt(&user, secret).unwrap();\n+    let claims = validate_jwt(&token, secret).unwrap();\n+    assert_eq!(claims.sub, user.id.to_string());",
        8, 5, "claude-1", "op-4", op4, "Edit");

    add_edit(&mut edits, &mut id, 570_000, "tests/auth_test.rs", EditKind::Modify,
        "@@ -20,0 +24,12 @@\n+#[test]\n+fn test_jwt_expiry() {\n+    let secret = b\"test-secret-key\";\n+    let user = create_test_user();\n+    let expired_claims = Claims {\n+        sub: user.id.to_string(),\n+        exp: 0, // already expired\n+        iat: 0,\n+        role: \"user\".to_string(),\n+    };\n+    // ... test expired token handling\n+}",
        12, 0, "claude-1", "op-4", op4, "Edit");

    for i in 0..4 {
        add_edit(&mut edits, &mut id, 590_000 + i * 15_000, "tests/auth_test.rs", EditKind::Modify,
            &format!("@@ -{},{} +{},{} @@\n+#[test]\n+fn test_auth_case_{}() {{\n+    // test case {}\n+}}", 36+i*8, 0, 36+i*8, 4, i, i),
            4, 0, "claude-1", "op-4", op4, "Edit");
    }

    // ── Op 5: Fix refresh tokens (5 edits, 660-780s) ────────────────────────
    let op5 = "Fix refresh token handling";

    add_edit(&mut edits, &mut id, 665_000, "src/api/refresh.rs", EditKind::Modify,
        "@@ -5,6 +5,14 @@\n-pub async fn refresh(req: RefreshRequest) -> Result<TokenResponse, ApiError> {\n-    let session = validate_refresh_token(&req.token)?;\n-    let new_token = rotate_session(&session)?;\n-    Ok(TokenResponse { token: new_token })\n+pub async fn refresh(req: RefreshRequest) -> Result<TokenResponse, ApiError> {\n+    let config = get_config();\n+    let claims = validate_jwt(&req.refresh_token, config.jwt_refresh_secret.as_bytes())\n+        .map_err(|_| ApiError::InvalidRefreshToken)?;\n+    let user = get_user_by_id(&claims.sub).await?;\n+    let access_token = create_jwt(&user, config.jwt_secret.as_bytes())?;\n+    let refresh_token = create_refresh_token(&user, config.jwt_refresh_secret.as_bytes())?;\n+    Ok(TokenResponse {\n+        access_token,\n+        refresh_token,\n+        expires_in: config.jwt_expiry_secs,\n+    })\n }",
        12, 4, "claude-2", "op-5", op5, "Edit");

    add_edit(&mut edits, &mut id, 690_000, "src/session.rs", EditKind::Modify,
        "@@ -12,4 +12,10 @@\n-pub fn get_session_ttl() -> Duration {\n-    Duration::from_secs(3600)\n+pub fn get_token_ttl(token_type: TokenType) -> Duration {\n+    match token_type {\n+        TokenType::Access => Duration::from_secs(900),   // 15 minutes\n+        TokenType::Refresh => Duration::from_secs(604800), // 7 days\n+    }\n }",
        5, 2, "claude-2", "op-5", op5, "Edit");

    add_edit(&mut edits, &mut id, 710_000, "src/session.rs", EditKind::Modify,
        "@@ -1,2 +1,5 @@\n use std::time::Duration;\n+\n+pub enum TokenType {\n+    Access,\n+    Refresh,\n }",
        4, 0, "claude-2", "op-5", op5, "Edit");

    add_edit(&mut edits, &mut id, 730_000, "src/api/refresh.rs", EditKind::Modify,
        "@@ -20,0 +28,6 @@\n+pub async fn revoke(req: RevokeRequest) -> Result<(), ApiError> {\n+    let config = get_config();\n+    // Add token to blocklist\n+    add_to_blocklist(&req.token, get_token_ttl(TokenType::Refresh)).await?;\n+    Ok(())\n+}",
        6, 0, "claude-2", "op-5", op5, "Edit");

    add_edit(&mut edits, &mut id, 750_000, "src/api/refresh.rs", EditKind::Modify,
        "@@ -1,2 +1,4 @@\n use crate::auth::{validate_jwt, create_jwt, create_refresh_token};\n+use crate::session::{get_token_ttl, TokenType};\n+use crate::blocklist::add_to_blocklist;\n use crate::config::get_config;",
        2, 0, "claude-2", "op-5", op5, "Edit");

    // ── Op 6: Update docs (3 edits, 780-900s) ───────────────────────────────
    let op6 = "Update documentation";

    add_edit(&mut edits, &mut id, 790_000, "README.md", EditKind::Modify,
        "@@ -45,6 +45,14 @@\n ## Authentication\n \n-This project uses session-based authentication.\n-Tokens are stored server-side and validated on each request.\n+This project uses JWT-based authentication with refresh tokens.\n+\n+- Access tokens expire after 15 minutes\n+- Refresh tokens expire after 7 days\n+- Rate limiting: 100 requests per minute per IP\n+\n+### Token Flow\n+\n+1. POST /login -> { access_token, refresh_token }\n+2. Use access_token in Authorization: Bearer header\n+3. POST /refresh when access_token expires",
        12, 2, "claude-1", "op-6", op6, "Edit");

    add_edit(&mut edits, &mut id, 830_000, "Cargo.toml", EditKind::Modify,
        "@@ -3,1 +3,1 @@\n-version = \"0.4.2\"\n+version = \"0.5.0\"",
        1, 1, "claude-1", "op-6", op6, "Edit");

    add_edit(&mut edits, &mut id, 860_000, "src/main.rs", EditKind::Modify,
        "@@ -1,2 +1,4 @@\n use auth::validate_jwt;\n+use middleware::RateLimiter;\n+use config::Config;\n \n fn main() {",
        2, 0, "claude-1", "op-6", op6, "Edit");

    edits
}

/// Generate synthetic conversation turns.
fn generate_conversation_turns(session_start_ms: i64) -> Vec<ConversationTurn> {
    vec![
        ConversationTurn {
            timestamp: session_start_ms + 2_000,
            user_prompt: "refactor the auth system to use JWT instead of session tokens".to_string(),
            tool_calls: vec![
                ToolCall { tool_name: "Read".to_string(), file_path: Some("src/auth.rs".to_string()), lines_added: None, lines_removed: None, timestamp: session_start_ms + 3_000, result_summary: "142 lines".to_string() },
                ToolCall { tool_name: "Read".to_string(), file_path: Some("src/middleware.rs".to_string()), lines_added: None, lines_removed: None, timestamp: session_start_ms + 4_000, result_summary: "89 lines".to_string() },
                ToolCall { tool_name: "Grep".to_string(), file_path: None, lines_added: None, lines_removed: None, timestamp: session_start_ms + 5_000, result_summary: "12 matches for session_token".to_string() },
                ToolCall { tool_name: "Edit".to_string(), file_path: Some("src/auth.rs".to_string()), lines_added: Some(14), lines_removed: Some(8), timestamp: session_start_ms + 65_000, result_summary: String::new() },
                ToolCall { tool_name: "Edit".to_string(), file_path: Some("src/middleware.rs".to_string()), lines_added: Some(22), lines_removed: Some(3), timestamp: session_start_ms + 110_000, result_summary: String::new() },
                ToolCall { tool_name: "Edit".to_string(), file_path: Some("src/config.rs".to_string()), lines_added: Some(4), lines_removed: Some(1), timestamp: session_start_ms + 148_000, result_summary: String::new() },
            ],
            assistant_text: "I've refactored the auth system to use JWT. The main changes are...".to_string(),
            tokens_in: 8200,
            tokens_out: 3400,
            cache_read: 2800,
            cache_write: 1200,
            model: "claude-opus-4-6".to_string(),
            duration_ms: 180_000,
        },
        ConversationTurn {
            timestamp: session_start_ms + 360_000,
            user_prompt: "add rate limiting to the login endpoint".to_string(),
            tool_calls: vec![
                ToolCall { tool_name: "Read".to_string(), file_path: Some("src/api/login.rs".to_string()), lines_added: None, lines_removed: None, timestamp: session_start_ms + 361_000, result_summary: "45 lines".to_string() },
                ToolCall { tool_name: "Edit".to_string(), file_path: Some("src/middleware.rs".to_string()), lines_added: Some(18), lines_removed: Some(0), timestamp: session_start_ms + 380_000, result_summary: String::new() },
                ToolCall { tool_name: "Edit".to_string(), file_path: Some("src/config.rs".to_string()), lines_added: Some(4), lines_removed: Some(0), timestamp: session_start_ms + 400_000, result_summary: String::new() },
                ToolCall { tool_name: "Edit".to_string(), file_path: Some("src/api/login.rs".to_string()), lines_added: Some(6), lines_removed: Some(0), timestamp: session_start_ms + 420_000, result_summary: String::new() },
            ],
            assistant_text: "I've added a RateLimiter middleware with configurable limits...".to_string(),
            tokens_in: 12400,
            tokens_out: 4200,
            cache_read: 5600,
            cache_write: 800,
            model: "claude-opus-4-6".to_string(),
            duration_ms: 120_000,
        },
        ConversationTurn {
            timestamp: session_start_ms + 540_000,
            user_prompt: "now update all the tests".to_string(),
            tool_calls: vec![
                ToolCall { tool_name: "Read".to_string(), file_path: Some("tests/auth_test.rs".to_string()), lines_added: None, lines_removed: None, timestamp: session_start_ms + 541_000, result_summary: "68 lines".to_string() },
                ToolCall { tool_name: "Edit".to_string(), file_path: Some("tests/auth_test.rs".to_string()), lines_added: Some(42), lines_removed: Some(18), timestamp: session_start_ms + 545_000, result_summary: String::new() },
                ToolCall { tool_name: "Edit".to_string(), file_path: Some("tests/auth_test.rs".to_string()), lines_added: Some(8), lines_removed: Some(2), timestamp: session_start_ms + 570_000, result_summary: String::new() },
                ToolCall { tool_name: "Bash".to_string(), file_path: None, lines_added: None, lines_removed: None, timestamp: session_start_ms + 640_000, result_summary: "test result: ok. 12 passed".to_string() },
            ],
            assistant_text: "All tests have been updated for JWT auth. Running cargo test...".to_string(),
            tokens_in: 9800,
            tokens_out: 2800,
            cache_read: 4200,
            cache_write: 600,
            model: "claude-opus-4-6".to_string(),
            duration_ms: 100_000,
        },
        ConversationTurn {
            timestamp: session_start_ms + 660_000,
            user_prompt: "fix the refresh token issue - tokens expire too quickly".to_string(),
            tool_calls: vec![
                ToolCall { tool_name: "Read".to_string(), file_path: Some("src/session.rs".to_string()), lines_added: None, lines_removed: None, timestamp: session_start_ms + 661_000, result_summary: "34 lines".to_string() },
                ToolCall { tool_name: "Edit".to_string(), file_path: Some("src/api/refresh.rs".to_string()), lines_added: Some(15), lines_removed: Some(6), timestamp: session_start_ms + 665_000, result_summary: String::new() },
                ToolCall { tool_name: "Edit".to_string(), file_path: Some("src/session.rs".to_string()), lines_added: Some(8), lines_removed: Some(3), timestamp: session_start_ms + 690_000, result_summary: String::new() },
            ],
            assistant_text: "Fixed refresh token TTL. Access tokens now expire after 15min...".to_string(),
            tokens_in: 7600,
            tokens_out: 1400,
            cache_read: 3800,
            cache_write: 400,
            model: "claude-opus-4-6".to_string(),
            duration_ms: 100_000,
        },
        ConversationTurn {
            timestamp: session_start_ms + 780_000,
            user_prompt: "update the docs and bump the version".to_string(),
            tool_calls: vec![
                ToolCall { tool_name: "Edit".to_string(), file_path: Some("README.md".to_string()), lines_added: Some(12), lines_removed: Some(4), timestamp: session_start_ms + 790_000, result_summary: String::new() },
                ToolCall { tool_name: "Edit".to_string(), file_path: Some("Cargo.toml".to_string()), lines_added: Some(1), lines_removed: Some(1), timestamp: session_start_ms + 830_000, result_summary: String::new() },
                ToolCall { tool_name: "Edit".to_string(), file_path: Some("src/main.rs".to_string()), lines_added: Some(3), lines_removed: Some(1), timestamp: session_start_ms + 860_000, result_summary: String::new() },
            ],
            assistant_text: "Documentation updated and version bumped to 0.5.0.".to_string(),
            tokens_in: 7200,
            tokens_out: 1300,
            cache_read: 3200,
            cache_write: 200,
            model: "claude-opus-4-6".to_string(),
            duration_ms: 90_000,
        },
    ]
}

/// Map each edit's after_hash to realistic file content so the preview pane
/// can display actual code in demo mode (no snapshot store on disk).
fn populate_synthetic_content(app: &mut App) {
    // Build a lookup: for each file, collect the after_hashes in order.
    // We generate progressive versions of the code so scrubbing through
    // shows the file evolving.
    let mut file_hashes: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for edit in &app.edits {
        file_hashes
            .entry(edit.file.clone())
            .or_default()
            .push(edit.after_hash.clone());
    }

    // For each file, generate content versions and map hashes.
    for (file, hashes) in &file_hashes {
        let versions = generate_file_versions(file, hashes.len());
        for (hash, content) in hashes.iter().zip(versions.into_iter()) {
            app.synthetic_content.insert(hash.clone(), content);
        }
    }
}

/// Generate progressive versions of a file's content. Returns one String
/// per version (one per edit to that file).
fn generate_file_versions(filename: &str, count: usize) -> Vec<String> {
    match filename {
        "src/auth.rs" => gen_auth_versions(count),
        "src/middleware.rs" => gen_middleware_versions(count),
        "src/config.rs" => gen_config_versions(count),
        "src/api/login.rs" => gen_login_versions(count),
        "tests/auth_test.rs" => gen_test_versions(count),
        "src/api/refresh.rs" => gen_refresh_versions(count),
        "src/session.rs" => gen_session_versions(count),
        "README.md" => gen_readme_versions(count),
        "Cargo.toml" => gen_cargo_versions(count),
        "src/main.rs" => gen_main_versions(count),
        _ => vec!["// unknown file\n".to_string(); count],
    }
}

fn gen_auth_versions(count: usize) -> Vec<String> {
    let v1 = r#"use std::collections::HashMap;
use std::time::Duration;
use serde::{Serialize, Deserialize};

pub struct AuthConfig {
    pub session_ttl: u64,
    pub max_retries: u32,
}

/// Validate a session token against the in-memory store.
pub fn validate_session(token: &str) -> Result<User, AuthError> {
    let session = SESSION_STORE.get(token)?;
    if session.is_expired() {
        return Err(AuthError::Expired);
    }
    Ok(session.user.clone())
}

/// Create a new session for a user and return the token.
pub fn create_session(user: &User) -> String {
    let token = generate_random_token();
    SESSION_STORE.insert(token.clone(), Session::new(user));
    token
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: u64,
    pub username: String,
    pub role: String,
}

#[derive(Debug)]
pub enum AuthError {
    Expired,
    InvalidToken,
    NotAuthenticated,
    Internal(String),
}
"#;

    let v2 = r#"use std::collections::HashMap;
use std::time::Duration;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation, Algorithm};
use jsonwebtoken::errors::ErrorKind;
use serde::{Serialize, Deserialize};

pub struct AuthConfig {
    pub session_ttl: u64,
    pub max_retries: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub role: String,
}

/// Validate a JWT token and extract claims.
pub fn validate_jwt(token: &str, secret: &[u8]) -> Result<Claims, AuthError> {
    let key = DecodingKey::from_secret(secret);
    let validation = Validation::new(Algorithm::HS256);
    let token_data = decode::<Claims>(token, &key, &validation)
        .map_err(|e| match e.kind() {
            ErrorKind::ExpiredSignature => AuthError::Expired,
            ErrorKind::InvalidToken => AuthError::InvalidToken,
            _ => AuthError::Internal(e.to_string()),
        })?;
    Ok(token_data.claims)
}

/// Create a new JWT for a user.
pub fn create_jwt(user: &User, secret: &[u8]) -> Result<String, AuthError> {
    let now = chrono::Utc::now();
    let claims = Claims {
        sub: user.id.to_string(),
        exp: (now + chrono::Duration::hours(24)).timestamp() as usize,
        iat: now.timestamp() as usize,
        role: user.role.to_string(),
    };
    let key = EncodingKey::from_secret(secret);
    encode(&Header::default(), &claims, &key)
        .map_err(|e| AuthError::Internal(e.to_string()))
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: u64,
    pub username: String,
    pub role: String,
}

#[derive(Debug)]
pub enum AuthError {
    Expired,
    InvalidToken,
    MissingToken,
    InvalidFormat,
    NotAuthenticated,
    Internal(String),
}
"#;

    let v3 = r#"use std::collections::HashMap;
use std::time::Duration;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation, Algorithm};
use jsonwebtoken::errors::ErrorKind;
use serde::{Serialize, Deserialize};

pub struct AuthConfig {
    pub session_ttl: u64,
    pub max_retries: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub role: String,
}

/// Validate a JWT token and extract claims.
pub fn validate_jwt(token: &str, secret: &[u8]) -> Result<Claims, AuthError> {
    let key = DecodingKey::from_secret(secret);
    let validation = Validation::new(Algorithm::HS256);
    let token_data = decode::<Claims>(token, &key, &validation)
        .map_err(|e| match e.kind() {
            ErrorKind::ExpiredSignature => AuthError::Expired,
            ErrorKind::InvalidToken => AuthError::InvalidToken,
            _ => AuthError::Internal(e.to_string()),
        })?;
    Ok(token_data.claims)
}

/// Create a new JWT for a user.
pub fn create_jwt(user: &User, secret: &[u8]) -> Result<String, AuthError> {
    let now = chrono::Utc::now();
    let claims = Claims {
        sub: user.id.to_string(),
        exp: (now + chrono::Duration::hours(24)).timestamp() as usize,
        iat: now.timestamp() as usize,
        role: user.role.to_string(),
    };
    let key = EncodingKey::from_secret(secret);
    encode(&Header::default(), &claims, &key)
        .map_err(|e| AuthError::Internal(e.to_string()))
}

/// Create a refresh token with a longer TTL.
pub fn create_refresh_token(user: &User, secret: &[u8]) -> Result<String, AuthError> {
    let now = chrono::Utc::now();
    let claims = Claims {
        sub: user.id.to_string(),
        exp: (now + chrono::Duration::days(7)).timestamp() as usize,
        iat: now.timestamp() as usize,
        role: user.role.to_string(),
    };
    let key = EncodingKey::from_secret(secret);
    encode(&Header::default(), &claims, &key)
        .map_err(|e| AuthError::Internal(e.to_string()))
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: u64,
    pub username: String,
    pub role: String,
}

#[derive(Debug)]
pub enum AuthError {
    Expired,
    InvalidToken,
    MissingToken,
    InvalidFormat,
    NotAuthenticated,
    Internal(String),
}
"#;

    interpolate_versions(&[v1, v2, v3], count)
}

fn gen_middleware_versions(count: usize) -> Vec<String> {
    let v1 = r#"use std::collections::HashMap;
use crate::auth::{validate_session, AuthError};

/// Authentication middleware: validates session tokens on each request.
pub fn auth_middleware(req: &Request) -> Result<(), AuthError> {
    let token = req.header("Authorization");
    // TODO: migrate to JWT validation
    // Current: session token lookup
    validate_session(token)
}
"#;

    let v2 = r#"use std::collections::HashMap;
use crate::auth::{validate_jwt, Claims, AuthError};

/// Authentication middleware: validates JWT on each request.
pub fn auth_middleware(req: &Request) -> Result<(), AuthError> {
    let auth_header = req.header("Authorization")
        .ok_or(AuthError::MissingToken)?;
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(AuthError::InvalidFormat)?;
    let secret = req.state().config.jwt_secret.as_bytes();
    let claims = validate_jwt(token, secret)?;
    req.set_extension(claims);
    Ok(())
}

/// Extract claims from a previously-authenticated request.
pub fn extract_claims(req: &Request) -> Result<&Claims, AuthError> {
    req.extensions()
        .get::<Claims>()
        .ok_or(AuthError::NotAuthenticated)
}
"#;

    let v3 = r#"use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Duration;
use crate::auth::{validate_jwt, Claims, AuthError};

/// Authentication middleware: validates JWT on each request.
pub fn auth_middleware(req: &Request) -> Result<(), AuthError> {
    let auth_header = req.header("Authorization")
        .ok_or(AuthError::MissingToken)?;
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(AuthError::InvalidFormat)?;
    let secret = req.state().config.jwt_secret.as_bytes();
    let claims = validate_jwt(token, secret)?;
    req.set_extension(claims);
    Ok(())
}

/// Extract claims from a previously-authenticated request.
pub fn extract_claims(req: &Request) -> Result<&Claims, AuthError> {
    req.extensions()
        .get::<Claims>()
        .ok_or(AuthError::NotAuthenticated)
}

pub struct RateLimiter {
    limits: Arc<RwLock<HashMap<String, (u64, std::time::Instant)>>>,
    max_requests: u64,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: u64, window: Duration) -> Self {
        Self {
            limits: Arc::new(RwLock::new(HashMap::new())),
            max_requests,
            window,
        }
    }

    pub async fn check(&self, ip: &str) -> Result<(), AuthError> {
        let mut limits = self.limits.write().await;
        let now = std::time::Instant::now();

        let entry = limits.entry(ip.to_string()).or_insert((0, now));
        if now.duration_since(entry.1) > self.window {
            *entry = (1, now);
            return Ok(());
        }

        entry.0 += 1;
        if entry.0 > self.max_requests {
            return Err(AuthError::Internal("rate limited".into()));
        }
        Ok(())
    }
}
"#;

    interpolate_versions(&[v1, v2, v3], count)
}

fn gen_config_versions(count: usize) -> Vec<String> {
    let v1 = r#"use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub db_url: String,
    pub jwt_secret: Option<String>,
    pub jwt_expiry_secs: u64,
    pub host: String,
    pub port: u16,
    pub log_level: String,
}

pub fn get_config() -> &'static Config {
    CONFIG.get().expect("config not initialized")
}

static CONFIG: std::sync::OnceLock<Config> = std::sync::OnceLock::new();
"#;

    let v2 = r#"use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub db_url: String,
    pub jwt_secret: String,
    pub jwt_refresh_secret: String,
    pub jwt_expiry_secs: u64,
    pub jwt_refresh_expiry_secs: u64,
    pub host: String,
    pub port: u16,
    pub log_level: String,
}

pub fn get_config() -> &'static Config {
    CONFIG.get().expect("config not initialized")
}

static CONFIG: std::sync::OnceLock<Config> = std::sync::OnceLock::new();
"#;

    let v3 = r#"use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub db_url: String,
    pub jwt_secret: String,
    pub jwt_refresh_secret: String,
    pub jwt_expiry_secs: u64,
    pub jwt_refresh_expiry_secs: u64,
    pub rate_limit_requests: u64,
    pub rate_limit_window_secs: u64,
    pub rate_limit_burst: u64,
    pub rate_limit_enabled: bool,
    pub host: String,
    pub port: u16,
    pub log_level: String,
}

pub const MAX_RETRIES: u32 = 5;

pub fn get_config() -> &'static Config {
    CONFIG.get().expect("config not initialized")
}

static CONFIG: std::sync::OnceLock<Config> = std::sync::OnceLock::new();
"#;

    interpolate_versions(&[v1, v2, v3], count)
}

fn gen_login_versions(count: usize) -> Vec<String> {
    let v1 = r#"use crate::auth::{create_jwt, create_refresh_token, AuthError};
use crate::config::get_config;

#[derive(Debug, serde::Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, serde::Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

pub async fn login(req: LoginRequest) -> Result<LoginResponse, ApiError> {
    let user = authenticate(&req.username, &req.password).await?;
    let config = get_config();
    let access_token = create_jwt(&user, config.jwt_secret.as_bytes())?;
    let refresh_token = create_refresh_token(&user, config.jwt_refresh_secret.as_bytes())?;
    Ok(LoginResponse {
        access_token,
        refresh_token,
        expires_in: config.jwt_expiry_secs,
    })
}
"#;

    let v2 = r#"use crate::auth::{create_jwt, create_refresh_token, AuthError};
use crate::config::get_config;
use crate::middleware::RateLimiter;

#[derive(Debug, serde::Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, serde::Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

pub async fn login(req: LoginRequest) -> Result<LoginResponse, ApiError> {
    let user = authenticate(&req.username, &req.password).await?;
    let config = get_config();
    let access_token = create_jwt(&user, config.jwt_secret.as_bytes())?;
    let refresh_token = create_refresh_token(&user, config.jwt_refresh_secret.as_bytes())?;
    Ok(LoginResponse {
        access_token,
        refresh_token,
        expires_in: config.jwt_expiry_secs,
    })
}

pub async fn rate_limited_login(req: LoginRequest, limiter: &RateLimiter) -> Result<LoginResponse, ApiError> {
    let ip = req.remote_addr();
    limiter.check(ip).await.map_err(|_| ApiError::RateLimited)?;
    login(req).await
}
"#;

    interpolate_versions(&[v1, v2], count)
}

fn gen_test_versions(count: usize) -> Vec<String> {
    let v1 = r#"use crate::auth::{validate_jwt, create_jwt, Claims};

fn create_test_user() -> crate::auth::User {
    crate::auth::User {
        id: 42,
        username: "testuser".to_string(),
        role: "admin".to_string(),
    }
}

#[test]
fn test_jwt_validation() {
    let secret = b"test-secret-key";
    let user = create_test_user();
    let token = create_jwt(&user, secret).unwrap();
    let claims = validate_jwt(&token, secret).unwrap();
    assert_eq!(claims.sub, user.id.to_string());
    assert_eq!(claims.role, "admin");
}

#[test]
fn test_jwt_expiry() {
    let secret = b"test-secret-key";
    let expired_token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &Claims {
            sub: "42".to_string(),
            exp: 0,
            iat: 0,
            role: "user".to_string(),
        },
        &jsonwebtoken::EncodingKey::from_secret(secret),
    )
    .unwrap();
    let result = validate_jwt(&expired_token, secret);
    assert!(result.is_err());
}
"#;

    let v2 = r#"use crate::auth::{validate_jwt, create_jwt, create_refresh_token, Claims};

fn create_test_user() -> crate::auth::User {
    crate::auth::User {
        id: 42,
        username: "testuser".to_string(),
        role: "admin".to_string(),
    }
}

#[test]
fn test_jwt_validation() {
    let secret = b"test-secret-key";
    let user = create_test_user();
    let token = create_jwt(&user, secret).unwrap();
    let claims = validate_jwt(&token, secret).unwrap();
    assert_eq!(claims.sub, user.id.to_string());
    assert_eq!(claims.role, "admin");
}

#[test]
fn test_jwt_expiry() {
    let secret = b"test-secret-key";
    let expired_token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &Claims {
            sub: "42".to_string(),
            exp: 0,
            iat: 0,
            role: "user".to_string(),
        },
        &jsonwebtoken::EncodingKey::from_secret(secret),
    )
    .unwrap();
    let result = validate_jwt(&expired_token, secret);
    assert!(result.is_err());
}

#[test]
fn test_wrong_secret() {
    let user = create_test_user();
    let token = create_jwt(&user, b"secret-1").unwrap();
    let result = validate_jwt(&token, b"secret-2");
    assert!(result.is_err());
}

#[test]
fn test_refresh_token_creation() {
    let secret = b"refresh-secret";
    let user = create_test_user();
    let token = create_refresh_token(&user, secret).unwrap();
    let claims = validate_jwt(&token, secret).unwrap();
    assert_eq!(claims.sub, "42");
}

#[test]
fn test_claims_role_preserved() {
    let secret = b"test-secret";
    let user = create_test_user();
    let token = create_jwt(&user, secret).unwrap();
    let claims = validate_jwt(&token, secret).unwrap();
    assert_eq!(claims.role, user.role);
}

#[test]
fn test_multiple_users() {
    let secret = b"test-secret";
    let user_a = crate::auth::User { id: 1, username: "alice".into(), role: "admin".into() };
    let user_b = crate::auth::User { id: 2, username: "bob".into(), role: "viewer".into() };
    let token_a = create_jwt(&user_a, secret).unwrap();
    let token_b = create_jwt(&user_b, secret).unwrap();
    assert_ne!(token_a, token_b);
    assert_eq!(validate_jwt(&token_a, secret).unwrap().role, "admin");
    assert_eq!(validate_jwt(&token_b, secret).unwrap().role, "viewer");
}
"#;

    interpolate_versions(&[v1, v2], count)
}

fn gen_refresh_versions(count: usize) -> Vec<String> {
    let v1 = r#"use crate::auth::{validate_jwt, create_jwt, create_refresh_token};
use crate::config::get_config;

pub async fn refresh(req: RefreshRequest) -> Result<TokenResponse, ApiError> {
    let config = get_config();
    let claims = validate_jwt(&req.refresh_token, config.jwt_refresh_secret.as_bytes())
        .map_err(|_| ApiError::InvalidRefreshToken)?;
    let user = get_user_by_id(&claims.sub).await?;
    let access_token = create_jwt(&user, config.jwt_secret.as_bytes())?;
    let refresh_token = create_refresh_token(&user, config.jwt_refresh_secret.as_bytes())?;
    Ok(TokenResponse {
        access_token,
        refresh_token,
        expires_in: config.jwt_expiry_secs,
    })
}
"#;

    let v2 = r#"use crate::auth::{validate_jwt, create_jwt, create_refresh_token};
use crate::session::{get_token_ttl, TokenType};
use crate::blocklist::add_to_blocklist;
use crate::config::get_config;

pub async fn refresh(req: RefreshRequest) -> Result<TokenResponse, ApiError> {
    let config = get_config();
    let claims = validate_jwt(&req.refresh_token, config.jwt_refresh_secret.as_bytes())
        .map_err(|_| ApiError::InvalidRefreshToken)?;
    let user = get_user_by_id(&claims.sub).await?;
    let access_token = create_jwt(&user, config.jwt_secret.as_bytes())?;
    let refresh_token = create_refresh_token(&user, config.jwt_refresh_secret.as_bytes())?;
    Ok(TokenResponse {
        access_token,
        refresh_token,
        expires_in: config.jwt_expiry_secs,
    })
}

pub async fn revoke(req: RevokeRequest) -> Result<(), ApiError> {
    let config = get_config();
    add_to_blocklist(&req.token, get_token_ttl(TokenType::Refresh)).await?;
    Ok(())
}
"#;

    interpolate_versions(&[v1, v2], count)
}

fn gen_session_versions(count: usize) -> Vec<String> {
    let v1 = r#"use std::time::Duration;

pub enum TokenType {
    Access,
    Refresh,
}

pub fn get_token_ttl(token_type: TokenType) -> Duration {
    match token_type {
        TokenType::Access => Duration::from_secs(900),     // 15 minutes
        TokenType::Refresh => Duration::from_secs(604800), // 7 days
    }
}
"#;

    interpolate_versions(&[v1], count)
}

fn gen_readme_versions(count: usize) -> Vec<String> {
    let v1 = r#"# Auth Service

A lightweight authentication service built with Rust.

## Authentication

This project uses JWT-based authentication with refresh tokens.

- Access tokens expire after 15 minutes
- Refresh tokens expire after 7 days
- Rate limiting: 100 requests per minute per IP

### Token Flow

1. POST /login -> { access_token, refresh_token }
2. Use access_token in Authorization: Bearer header
3. POST /refresh when access_token expires

## Getting Started

```bash
cargo run --release
```

## Configuration

Set environment variables or use `config.toml`:

```toml
jwt_secret = "your-secret-key"
jwt_refresh_secret = "your-refresh-secret"
jwt_expiry_secs = 900
jwt_refresh_expiry_secs = 604800
rate_limit_requests = 100
rate_limit_window_secs = 60
```
"#;

    interpolate_versions(&[v1], count)
}

fn gen_cargo_versions(count: usize) -> Vec<String> {
    let v1 = r#"[package]
name = "auth-service"
version = "0.5.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
jsonwebtoken = "9"
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
"#;

    interpolate_versions(&[v1], count)
}

fn gen_main_versions(count: usize) -> Vec<String> {
    let v1 = r#"use auth::validate_jwt;
use middleware::RateLimiter;
use config::Config;

mod auth;
mod config;
mod middleware;
mod session;

fn main() {
    tracing_subscriber::init();
    let config = Config::from_env().expect("failed to load config");
    let limiter = RateLimiter::new(
        config.rate_limit_requests,
        std::time::Duration::from_secs(config.rate_limit_window_secs),
    );

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        tracing::info!("starting auth-service on {}:{}", config.host, config.port);
        serve(config, limiter).await
    });
}
"#;

    interpolate_versions(&[v1], count)
}

/// Given a set of version snapshots and a target count, distribute versions
/// evenly across the edits. Early edits get earlier versions, later edits
/// get later versions.
fn interpolate_versions(versions: &[&str], count: usize) -> Vec<String> {
    if count == 0 {
        return Vec::new();
    }
    if versions.len() == 1 || count == 1 {
        return vec![versions.last().unwrap_or(&"").to_string(); count];
    }
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        // Map edit index to a version index.
        let vi = i * (versions.len() - 1) / (count - 1).max(1);
        result.push(versions[vi.min(versions.len() - 1)].to_string());
    }
    result
}
