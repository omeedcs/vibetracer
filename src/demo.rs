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
