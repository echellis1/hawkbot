use std::{
    collections::BTreeMap,
    io,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::RwLock,
};
use tokio_serial::SerialPortBuilderExt;

use crate::{
    config::AppConfig,
    controllers::ActiveSport,
    rtd_state::data_source::serial_stream_data_source::SerialStreamDataSource,
    sports::{
        baseball::BaseballSport, basketball::BasketballSport, football::FootballSport,
        soccer::SoccerSport, volleyball::VolleyballSport, water_polo::WaterPoloSport,
        wrestling::WrestlingSport, Sport,
    },
    RTDState,
};

#[derive(Clone, Debug, serde::Serialize)]
struct RuntimeStatus {
    last_update: Option<String>,
    parser_status: String,
    controller_connection_state: String,
    latest_payload: Option<BTreeMap<String, String>>,
}

impl Default for RuntimeStatus {
    fn default() -> Self {
        Self {
            last_update: None,
            parser_status: "starting".into(),
            controller_connection_state: "disconnected".into(),
            latest_payload: None,
        }
    }
}

#[derive(Clone)]
pub struct WebState {
    config: Arc<RwLock<AppConfig>>,
    runtime: Arc<RwLock<RuntimeStatus>>,
    config_path: PathBuf,
}

pub async fn run(config: AppConfig, config_path: PathBuf) -> io::Result<()> {
    let bind = config.web_bind.clone();
    let state = WebState {
        config: Arc::new(RwLock::new(config)),
        runtime: Arc::new(RwLock::new(RuntimeStatus::default())),
        config_path,
    };

    let poll_state = state.clone();
    tokio::spawn(async move { poll_loop(poll_state).await });

    let listener = TcpListener::bind(&bind).await?;
    loop {
        let (stream, _) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            let _ = handle_connection(stream, state).await;
        });
    }
}

async fn poll_loop(state: WebState) {
    loop {
        let cfg = state.config.read().await.clone();
        state.runtime.write().await.controller_connection_state = "connecting".into();

        let port = match tokio_serial::new(cfg.serial_device.clone(), 19200)
            .parity(tokio_serial::Parity::None)
            .open_native_async()
        {
            Ok(p) => p,
            Err(err) => {
                let mut runtime = state.runtime.write().await;
                runtime.controller_connection_state = format!("error: {err}");
                runtime.parser_status = "disconnected".into();
                drop(runtime);
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        let mut parser = match DaktronicsParser::new(cfg.active_sport, port) {
            Ok(p) => p,
            Err(err) => {
                let mut runtime = state.runtime.write().await;
                runtime.parser_status = format!("error: {err}");
                runtime.controller_connection_state = "error".into();
                drop(runtime);
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        {
            let mut runtime = state.runtime.write().await;
            runtime.controller_connection_state = "connected".into();
            runtime.parser_status = "running".into();
        }

        loop {
            let cfg_now = state.config.read().await.clone();
            if cfg_now.active_sport != cfg.active_sport {
                break;
            }

            match parser.update_and_snapshot().await {
                Ok(Some(mut payload)) => {
                    payload.insert("sport".into(), cfg.active_sport.as_str().into());
                    payload.insert("controller".into(), "daktronics".into());
                    payload.insert("updated_at".into(), now_ts());
                    payload.insert("public_id".into(), cfg.public_status_uuid.clone());

                    let mut runtime = state.runtime.write().await;
                    runtime.last_update = Some(now_ts());
                    runtime.latest_payload = Some(payload);
                }
                Ok(None) => {}
                Err(err) => {
                    let mut runtime = state.runtime.write().await;
                    runtime.parser_status = format!("error: {err}");
                    runtime.controller_connection_state = "disconnected".into();
                    break;
                }
            }
        }
    }
}

fn now_ts() -> String {
    format!(
        "{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    )
}

enum DaktronicsParser {
    Baseball(BaseballSport<SerialStreamDataSource>),
    Basketball(BasketballSport<SerialStreamDataSource>),
    Football(FootballSport<SerialStreamDataSource>),
    Soccer(SoccerSport<SerialStreamDataSource>),
    Volleyball(VolleyballSport<SerialStreamDataSource>),
    Wrestling(WrestlingSport<SerialStreamDataSource>),
    WaterPolo(WaterPoloSport<SerialStreamDataSource>),
}

impl DaktronicsParser {
    fn new(
        sport: ActiveSport,
        port: tokio_serial::SerialStream,
    ) -> Result<Self, tokio_serial::Error> {
        let rtd_state = RTDState::from_serial_stream(port, true)?;
        Ok(match sport {
            ActiveSport::Baseball => Self::Baseball(BaseballSport::new(rtd_state)),
            ActiveSport::Basketball => Self::Basketball(BasketballSport::new(rtd_state)),
            ActiveSport::Football => Self::Football(FootballSport::new(rtd_state)),
            ActiveSport::Soccer => Self::Soccer(SoccerSport::new(rtd_state)),
            ActiveSport::Volleyball => Self::Volleyball(VolleyballSport::new(rtd_state)),
            ActiveSport::Wrestling => Self::Wrestling(WrestlingSport::new(rtd_state)),
            ActiveSport::WaterPolo => Self::WaterPolo(WaterPoloSport::new(rtd_state)),
        })
    }

    async fn update_and_snapshot(
        &mut self,
    ) -> Result<Option<BTreeMap<String, String>>, Box<dyn std::error::Error + Send + Sync>> {
        let changed = match self {
            Self::Baseball(s) => s.rtd_state().update_async().await?,
            Self::Basketball(s) => s.rtd_state().update_async().await?,
            Self::Football(s) => s.rtd_state().update_async().await?,
            Self::Soccer(s) => s.rtd_state().update_async().await?,
            Self::Volleyball(s) => s.rtd_state().update_async().await?,
            Self::Wrestling(s) => s.rtd_state().update_async().await?,
            Self::WaterPolo(s) => s.rtd_state().update_async().await?,
        };
        if !changed {
            return Ok(None);
        }

        let value = match self {
            Self::Baseball(s) => serde_json::to_value(s)?,
            Self::Basketball(s) => serde_json::to_value(s)?,
            Self::Football(s) => serde_json::to_value(s)?,
            Self::Soccer(s) => serde_json::to_value(s)?,
            Self::Volleyball(s) => serde_json::to_value(s)?,
            Self::Wrestling(s) => serde_json::to_value(s)?,
            Self::WaterPolo(s) => serde_json::to_value(s)?,
        };
        Ok(Some(flatten(value)))
    }
}

fn flatten(value: Value) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Value::Object(obj) = value {
        for (k, v) in obj {
            out.insert(
                to_snake_case(&k),
                match v {
                    Value::Null => String::new(),
                    Value::String(s) => s,
                    _ => v.to_string(),
                },
            );
        }
    }
    out
}

fn to_snake_case(s: &str) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn dashboard_html(public_uuid: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>Daktronics Gateway Dashboard</title>
<style>
:root {{
  --bg: #081d4c;
  --panel: #142b5a;
  --panel-border: #3a5586;
  --tile: #041a46;
  --text: #eaf1ff;
  --muted: #8ab2f7;
}}
* {{ box-sizing: border-box; }}
body {{
  margin: 0;
  font-family: Inter, system-ui, -apple-system, Segoe UI, Roboto, sans-serif;
  background: linear-gradient(180deg, #07205a, #081f50);
  color: var(--text);
  padding: 24px;
}}
.shell {{ max-width: 1120px; margin: 0 auto; }}
.header {{
  border: 1px solid var(--panel-border);
  border-radius: 16px;
  background: rgba(24, 44, 90, 0.95);
  padding: 16px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
}}
.title h1 {{ margin: 0; font-size: 39px; line-height: 1.1; }}
.title p {{ margin: 8px 0 0; color: var(--muted); font-size: 28px; }}
.btns {{ display: flex; gap: 12px; flex-wrap: wrap; }}
.btn {{
  border: 1px solid #4c80cf;
  border-radius: 12px;
  color: #eef4ff;
  text-decoration: none;
  padding: 10px 16px;
  font-size: 22px;
  font-weight: 600;
  background: #1d4f95;
}}
.board {{
  margin-top: 20px;
  border: 1px solid var(--panel-border);
  border-radius: 16px;
  background: var(--panel);
  padding: 16px;
}}
.score-grid {{
  display: grid;
  grid-template-columns: 1fr 0.35fr 1fr;
  gap: 12px;
}}
.card {{
  background: var(--tile);
  border: 1px solid #395789;
  border-radius: 14px;
  padding: 16px;
  min-height: 170px;
}}
.label {{ text-transform: uppercase; letter-spacing: 0.08em; color: #8ab2f7; font-size: 20px; }}
.name {{ font-size: 54px; font-weight: 700; margin-top: 8px; }}
.score {{ font-size: 80px; margin-top: 14px; line-height: 1; font-weight: 700; }}
.middle {{ text-align: center; display: flex; flex-direction: column; justify-content: center; }}
.middle .v {{ font-size: 56px; line-height: 1.1; font-weight: 700; }}
.details {{
  margin-top: 12px;
  display: grid;
  grid-template-columns: repeat(6, 1fr);
  gap: 10px;
  border: 1px solid #395789;
  border-radius: 14px;
  padding: 10px;
  background: rgba(7, 29, 74, 0.8);
}}
.detail {{
  border: 1px solid #395789;
  border-radius: 12px;
  background: #061c49;
  min-height: 100px;
  padding: 10px;
}}
.detail .k {{ text-transform: uppercase; color: #8ab2f7; font-size: 18px; letter-spacing: 0.06em; }}
.detail .v {{ font-size: 46px; margin-top: 6px; font-weight: 700; }}
@media (max-width: 980px) {{
  .title h1 {{ font-size: 30px; }}
  .title p {{ font-size: 18px; }}
  .score-grid {{ grid-template-columns: 1fr; }}
  .details {{ grid-template-columns: repeat(2, 1fr); }}
}}
</style>
</head>
<body>
  <div class="shell">
    <div class="header">
      <div class="title">
        <h1>Daktronics Gateway Dashboard</h1>
        <p>Live scoreboard relay and control surface</p>
      </div>
      <div class="btns">
        <a class="btn" href="/status/{0}.json" target="_blank">View Status JSON</a>
        <a class="btn" href="/admin">Open Admin Panel</a>
      </div>
    </div>

    <div class="board">
      <div class="score-grid">
        <div class="card">
          <div class="label">Home</div>
          <div id="home_team" class="name">Home Team</div>
          <div id="home_score" class="score">--</div>
        </div>
        <div class="card middle">
          <div class="label">Period</div>
          <div id="period" class="v">--</div>
          <div class="label" style="margin-top:8px;">Clock</div>
          <div id="clock" class="v">--:--</div>
        </div>
        <div class="card">
          <div class="label">Away</div>
          <div id="away_team" class="name">Away Team</div>
          <div id="away_score" class="score">--</div>
        </div>
      </div>

      <div class="details">
        <div class="detail"><div class="k">Home Timeouts</div><div id="home_timeouts" class="v">--</div></div>
        <div class="detail"><div class="k">Away Timeouts</div><div id="away_timeouts" class="v">--</div></div>
        <div class="detail"><div class="k">Detail 1</div><div id="detail_1" class="v">--</div></div>
        <div class="detail"><div class="k">Detail 2</div><div id="detail_2" class="v">--</div></div>
        <div class="detail"><div class="k">Detail 3</div><div id="detail_3" class="v">--</div></div>
        <div class="detail"><div class="k">Detail 6</div><div id="detail_6" class="v">--</div></div>
      </div>
    </div>
  </div>

<script>
const STATUS_URL = '/status/{0}.json';
const choose = (row, keys, fallback='--') => {{
  for (const k of keys) {{
    const v = row[k];
    if (v !== undefined && v !== null && String(v).trim() !== '') return String(v);
  }}
  return fallback;
}};

async function refresh() {{
  try {{
    const res = await fetch(STATUS_URL, {{ cache: 'no-store' }});
    if (!res.ok) return;
    const data = await res.json();
    const row = Array.isArray(data) ? (data[0] || {{}}) : {{}};

    document.getElementById('home_team').textContent = choose(row, ['home_team']);
    document.getElementById('away_team').textContent = choose(row, ['away_team']);
    document.getElementById('home_score').textContent = choose(row, ['home_score']);
    document.getElementById('away_score').textContent = choose(row, ['away_score']);
    document.getElementById('period').textContent = choose(row, ['period', 'quarter']);
    document.getElementById('clock').textContent = choose(row, ['clock', 'main_clock_time'], '--:--');
    document.getElementById('home_timeouts').textContent = choose(row, ['home_timeouts', 'home_time_outs_left_total']);
    document.getElementById('away_timeouts').textContent = choose(row, ['away_timeouts', 'guest_time_outs_left_total']);
    document.getElementById('detail_1').textContent = choose(row, ['possession', 'inning']);
    document.getElementById('detail_2').textContent = choose(row, ['balls', 'downs']);
    document.getElementById('detail_3').textContent = choose(row, ['strikes', 'yards_to_go']);
    document.getElementById('detail_6').textContent = choose(row, ['outs', 'fouls']);
  }} catch (_) {{}}
}}
setInterval(refresh, 1500);
refresh();
</script>
</body></html>"#,
        public_uuid
    )
}

async fn handle_connection(mut stream: TcpStream, state: WebState) -> io::Result<()> {
    let mut buf = [0; 4096];
    let n = stream.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }
    let req = String::from_utf8_lossy(&buf[..n]);
    let line = req.lines().next().unwrap_or("GET / HTTP/1.1");
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let target = parts.next().unwrap_or("/");

    let response = route(method, target, &req, state).await;
    stream.write_all(response.as_bytes()).await
}

async fn route(method: &str, target: &str, req: &str, state: WebState) -> String {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));

    if method == "GET" && path == "/" {
        let cfg = state.config.read().await.clone();
        let body = dashboard_html(&cfg.public_status_uuid);
        return http_ok("text/html", &body);
    }

    if method == "GET" && path.starts_with("/status/") && path.ends_with(".json") {
        let id = path
            .trim_start_matches("/status/")
            .trim_end_matches(".json");
        let cfg = state.config.read().await.clone();
        if id != cfg.public_status_uuid {
            return http_not_found();
        }
        let payload = state
            .runtime
            .read()
            .await
            .latest_payload
            .clone()
            .unwrap_or_default();
        let body = serde_json::to_string(&vec![payload]).unwrap_or_else(|_| "[]".into());
        return http_ok("application/json", &body);
    }

    if path.starts_with("/admin") && !authorized(query, &state).await {
        return http_unauthorized();
    }

    if method == "GET" && path == "/admin" {
        let body = "<html><body><h2>Admin</h2><form method='post' action='/admin/controller?token=admin'><input name='controller_type' value='daktronics'/><button>Set Controller</button></form><form method='post' action='/admin/sport?token=admin'><input name='active_sport' value='basketball'/><button>Set Sport</button></form><form method='post' action='/admin/rotate?token=admin'><button>Rotate URL</button></form><a href='/admin/config?token=admin'>View config</a></body></html>";
        return http_ok("text/html", body);
    }

    if method == "GET" && path == "/admin/config" {
        let cfg = state.config.read().await.clone();
        let rt = state.runtime.read().await.clone();
        let body = serde_json::json!({"config": cfg, "status": rt, "public_url": format!("/status/{}.json", cfg.public_status_uuid)}).to_string();
        return http_ok("application/json", &body);
    }

    if method == "POST" && path == "/admin/rotate" {
        let mut cfg = state.config.write().await;
        cfg.public_status_uuid = new_public_id();
        let _ = cfg.save(&state.config_path);
        return http_no_content();
    }

    let body_text = req.split("\r\n\r\n").nth(1).unwrap_or("");
    if method == "POST" && path == "/admin/controller" {
        let form = parse_form(body_text);
        if form.get("controller_type").map(String::as_str) == Some("daktronics") {
            let cfg = state.config.read().await.clone();
            let _ = cfg.save(&state.config_path);
            return http_no_content();
        }
        return http_bad_request();
    }

    if method == "POST" && path == "/admin/sport" {
        let form = parse_form(body_text);
        if let Some(s) = form.get("active_sport").and_then(|v| parse_sport(v)) {
            let mut cfg = state.config.write().await;
            cfg.active_sport = s;
            let _ = cfg.save(&state.config_path);
            return http_no_content();
        }
        return http_bad_request();
    }

    http_not_found()
}

async fn authorized(query: &str, state: &WebState) -> bool {
    let token = query
        .split('&')
        .find_map(|x| x.split_once('='))
        .filter(|(k, _)| *k == "token")
        .map(|(_, v)| v)
        .unwrap_or("");
    let cfg = state.config.read().await;
    token == cfg.admin_password_hash
}

fn parse_form(body: &str) -> BTreeMap<String, String> {
    body.split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.to_string(), v.replace('+', " ")))
        .collect()
}

fn parse_sport(s: &str) -> Option<ActiveSport> {
    Some(match s {
        "baseball" => ActiveSport::Baseball,
        "basketball" => ActiveSport::Basketball,
        "football" => ActiveSport::Football,
        "soccer" => ActiveSport::Soccer,
        "volleyball" => ActiveSport::Volleyball,
        "wrestling" => ActiveSport::Wrestling,
        "water_polo" => ActiveSport::WaterPolo,
        _ => return None,
    })
}

fn new_public_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let hex = format!("{nanos:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn http_ok(content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
}
fn http_no_content() -> String {
    "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".into()
}
fn http_not_found() -> String {
    "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".into()
}
fn http_bad_request() -> String {
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n".into()
}
fn http_unauthorized() -> String {
    "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n".into()
}

#[derive(Deserialize)]
struct _Never;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_conversion() {
        assert_eq!(to_snake_case("HomeScore"), "home_score");
    }

    #[test]
    fn uuid_shape() {
        let id = new_public_id();
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().filter(|c| *c == '-').count(), 4);
    }
}
