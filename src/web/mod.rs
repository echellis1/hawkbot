use std::{
    collections::{BTreeMap, HashSet},
    io,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime},
};

use serde::Deserialize;
use serde_json::{Map, Value};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::RwLock,
};
use tokio_serial::SerialPortBuilderExt;
use uuid::Uuid;

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
    latest_payload: Option<Value>,
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
    sessions: Arc<RwLock<HashSet<String>>>,
    config_path: PathBuf,
}

pub async fn run(config: AppConfig, config_path: PathBuf) -> io::Result<()> {
    let bind = config.web_bind.clone();
    let state = WebState {
        config: Arc::new(RwLock::new(config)),
        runtime: Arc::new(RwLock::new(RuntimeStatus::default())),
        sessions: Arc::new(RwLock::new(HashSet::new())),
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
                    if cfg.active_sport != ActiveSport::Basketball {
                        if let Value::Object(ref mut obj) = payload {
                            obj.insert("sport".into(), cfg.active_sport.as_str().into());
                            obj.insert("controller".into(), "daktronics".into());
                            obj.insert("updated_at".into(), now_ts().into());
                            obj.insert("public_id".into(), cfg.public_status_uuid.clone().into());
                        }
                    }

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
    format!("{:?}", SystemTime::now())
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
    ) -> Result<Option<Value>, Box<dyn std::error::Error + Send + Sync>> {
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
            Self::Baseball(s) => flatten(serde_json::to_value(s)?),
            Self::Basketball(s) => basketball_public_payload(s),
            Self::Football(s) => flatten(serde_json::to_value(s)?),
            Self::Soccer(s) => flatten(serde_json::to_value(s)?),
            Self::Volleyball(s) => flatten(serde_json::to_value(s)?),
            Self::Wrestling(s) => flatten(serde_json::to_value(s)?),
            Self::WaterPolo(s) => flatten(serde_json::to_value(s)?),
        };
        Ok(Some(with_common_public_fields(value)))
    }
}

fn with_common_public_fields(mut payload: Value) -> Value {
    let Value::Object(ref mut out) = payload else {
        return payload;
    };

    copy_first(
        out,
        "HomeTeamName",
        &["home_team_name", "home_team", "HomeTeamName"],
    );
    copy_first(
        out,
        "GuestTeamName",
        &["guest_team_name", "away_team", "GuestTeamName"],
    );
    copy_first(
        out,
        "HomeTeamScore",
        &["home_team_score", "home_score", "HomeTeamScore"],
    );
    copy_first(
        out,
        "GuestTeamScore",
        &["guest_team_score", "away_score", "GuestTeamScore"],
    );
    copy_first(out, "Period", &["period", "quarter", "Period"]);
    copy_first(
        out,
        "MainClockTime",
        &["main_clock_time", "clock", "MainClockTime"],
    );
    copy_first(
        out,
        "HomeTimeOutsLeftTotal",
        &[
            "home_time_outs_left_total",
            "home_timeouts",
            "HomeTimeOutsLeftTotal",
        ],
    );
    copy_first(
        out,
        "GuestTimeOutsLeftTotal",
        &[
            "guest_time_outs_left_total",
            "away_timeouts",
            "GuestTimeOutsLeftTotal",
        ],
    );

    payload
}

fn copy_first(out: &mut Map<String, Value>, target: &str, candidates: &[&str]) {
    if out.get(target).is_some() {
        return;
    }

    if let Some(v) = candidates.iter().find_map(|k| out.get(*k).cloned()) {
        out.insert(target.to_string(), v);
    }
}

fn flatten(value: Value) -> Value {
    let mut out = Map::new();
    if let Value::Object(obj) = value {
        for (k, v) in obj {
            out.insert(to_snake_case(&k), v);
        }
    }
    Value::Object(out)
}

fn basketball_public_payload(s: &BasketballSport<SerialStreamDataSource>) -> Value {
    let mut out = Map::new();

    macro_rules! insert_opt_str {
        ($k:literal, $v:expr) => {
            out.insert($k.into(), opt_str($v));
        };
    }
    macro_rules! insert_opt_i32 {
        ($k:literal, $v:expr) => {
            out.insert($k.into(), opt_i32($v));
        };
    }
    macro_rules! insert_bool {
        ($k:literal, $v:expr) => {
            out.insert($k.into(), Value::Bool($v.unwrap_or(false)));
        };
    }

    insert_opt_str!("MainClockTime", s.main_clock_time());
    insert_opt_str!("MainClockTime2", s.main_clock_time_2());
    insert_opt_str!("MainClockTimeOutTod", s.main_clock_time_out_tod());
    insert_opt_str!("MainClockTimeOutTod2", s.main_clock_time_out_tod_2());
    insert_bool!("MainClockIsZero", s.main_clock_is_zero());
    insert_bool!("MainClockStopped", s.main_clock_stopped());
    insert_bool!("MainClockTimeOutHorn", s.main_clock_time_out_horn());
    insert_bool!("MainClockHorn", s.main_clock_horn());
    insert_bool!("TimeOutHorn", s.time_out_horn());
    insert_opt_str!("TimeOutTime", s.time_out_time());
    insert_opt_str!("TimeOfDay", s.time_of_day());
    insert_opt_str!("HomeTeamName", s.home_team_name());
    insert_opt_str!("GuestTeamName", s.guest_team_name());
    insert_opt_i32!("HomeTeamScore", s.home_team_score());
    insert_opt_i32!("GuestTeamScore", s.guest_team_score());
    insert_opt_i32!("HomeTimeOutsLeftFull", s.home_time_outs_left_full());
    insert_opt_i32!("HomeTimeOutsLeftPartial", s.home_time_outs_left_partial());
    insert_opt_i32!("HomeTimeOutsLeftTotal", s.home_time_outs_left_total());
    insert_opt_i32!("GuestTimeOutsLeftFull", s.guest_time_outs_left_full());
    insert_opt_i32!("GuestTimeOutsLeftPartial", s.guest_time_outs_left_partial());
    insert_opt_i32!("GuestTimeOutsLeftTotal", s.guest_time_outs_left_total());
    insert_bool!("HomeTimeOutIndicator", s.home_time_out_indicator());
    insert_opt_str!("HomeTimeOutText", s.home_time_out_text());
    insert_bool!("GuestTimeOutIndicator", s.guest_time_out_indicator());
    insert_opt_str!("GuestTimeOutText", s.guest_time_out_text());
    insert_opt_i32!("Period", s.period());
    insert_opt_str!("InternalRelay", s.internal_relay());
    insert_bool!("AdPanelCaptionPower", s.ad_panel_caption_power());
    insert_bool!("AdPanelCaptionNum1", s.ad_panel_caption_num1());
    insert_bool!("AdPanelCaptionNum2", s.ad_panel_caption_num2());
    insert_opt_str!("ShotClockTime", s.shot_clock_time());
    insert_bool!("ShotClockHorn", s.shot_clock_horn());
    insert_bool!("HomePossessionIndicator", s.home_possession_indicator());
    insert_opt_str!("HomePossessionText", s.home_possession_text());
    insert_bool!("GuestPossessionIndicator", s.guest_possession_indicator());
    insert_opt_str!("GuestPossessionText", s.guest_possession_text());
    insert_bool!("Home1On1BonusIndicator", s.home_1_on_1_bonus_indicator());
    insert_bool!("Home2ShotBonusIndicator", s.home_2_shot_bonus_indicator());
    insert_opt_str!("HomeBonusText", s.home_bonus_text());
    insert_bool!("Guest1On1BonusIndicator", s.guest_1_on_1_bonus_indicator());
    insert_bool!("Guest2ShotBonusIndicator", s.guest_2_shot_bonus_indicator());
    insert_opt_str!("GuestBonusText", s.guest_bonus_text());
    insert_opt_i32!("HomeTeamFouls", s.home_team_fouls());
    insert_opt_i32!("GuestTeamFouls", s.guest_team_fouls());
    insert_opt_str!("HomePlayerFoulPoints", s.home_player_foul_points());
    insert_opt_str!("GuestPlayerFoulPoints", s.guest_player_foul_points());
    insert_opt_str!("PlayerFoul", s.player_foul());
    insert_opt_i32!("PlayerFoulPlayer", s.player_foul_player());
    insert_opt_i32!("PlayerFoulFoul", s.player_foul_foul());
    insert_opt_str!("PlayerFoulPoints", s.player_foul_points());
    insert_opt_i32!("HomeScorePeriod1", s.home_score_period_1());
    insert_opt_i32!("HomeScorePeriod2", s.home_score_period_2());
    insert_opt_i32!("HomeScorePeriod3", s.home_score_period_3());
    insert_opt_i32!("HomeScorePeriod4", s.home_score_period_4());
    insert_opt_i32!("HomeScorePeriod5", s.home_score_period_5());
    insert_opt_i32!("HomeScorePeriod7", s.home_score_period_7());
    insert_opt_i32!("HomeScorePeriod8", s.home_score_period_8());
    insert_opt_i32!("HomeScorePeriod9", s.home_score_period_9());
    insert_opt_i32!("HomeScoreCurrentPeriod", s.home_score_current_period());
    insert_opt_i32!("GuestScorePeriod1", s.guest_score_period_1());
    insert_opt_i32!("GuestScorePeriod2", s.guest_score_period_2());
    insert_opt_i32!("GuestScorePeriod3", s.guest_score_period_3());
    insert_opt_i32!("GuestScorePeriod4", s.guest_score_period_4());
    insert_opt_i32!("GuestScorePeriod5", s.guest_score_period_5());
    insert_opt_i32!("GuestScorePeriod6", s.guest_score_period_6());
    insert_opt_i32!("GuestScorePeriod7", s.guest_score_period_7());
    insert_opt_i32!("GuestScorePeriod8", s.guest_score_period_8());
    insert_opt_i32!("GuestScorePeriod9", s.guest_score_period_9());
    out.insert("updated_at".into(), now_ts().into());

    Value::Object(out)
}

fn opt_str(v: Result<&str, crate::rtd_state::RTDStateFieldError>) -> Value {
    match v {
        Ok(x) => Value::String(x.to_string()),
        Err(crate::rtd_state::RTDStateFieldError::NoData) => Value::Null,
        Err(_) => Value::Null,
    }
}

fn opt_i32(v: Result<i32, crate::rtd_state::RTDStateFieldError>) -> Value {
    match v {
        Ok(x) => Value::Number(x.into()),
        Err(crate::rtd_state::RTDStateFieldError::NoData) => Value::Null,
        Err(_) => Value::Null,
    }
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
  --bg: #081a3d;
  --panel: linear-gradient(180deg, #1b3567 0%, #182f5e 100%);
  --tile: linear-gradient(180deg, #1a3366 0%, #182f5c 100%);
  --border: #3b67a8;
  --text: #deebff;
  --muted: #9fb8df;
}}
* {{ box-sizing: border-box; }}
body {{
  margin: 0;
  font-family: Inter, system-ui, -apple-system, Segoe UI, Roboto, sans-serif;
  background-color: #061632;
  background-image: radial-gradient(circle at top left, #13366f 0%, #081a3d 52%, #061632 100%);
  background-repeat: no-repeat;
  background-size: cover;
  color: var(--text);
  padding: 24px;
  min-height: 100vh;
}}
.shell {{ max-width: 1120px; margin: 0 auto; }}
.header {{
  border: 1px solid var(--border);
  border-radius: 16px;
  background: var(--panel);
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
  border: 1px solid #4c7ec3;
  border-radius: 12px;
  color: #e8f1ff;
  text-decoration: none;
  padding: 10px 16px;
  font-size: 22px;
  font-weight: 600;
  background: linear-gradient(180deg, #2f62ab 0%, #2a5796 100%);
}}
.board {{
  margin-top: 20px;
  border: 1px solid var(--border);
  border-radius: 16px;
  background: linear-gradient(180deg, #193263 0%, #172d59 100%);
  padding: 16px;
}}
.score-grid {{
  display: grid;
  grid-template-columns: 1fr 0.35fr 1fr;
  gap: 12px;
}}
.card {{
  background: var(--tile);
  border: 1px solid var(--border);
  border-radius: 14px;
  padding: 16px;
  min-height: 170px;
}}
.label {{ text-transform: uppercase; letter-spacing: 0.08em; color: var(--muted); font-size: 20px; }}
.name {{ font-size: 54px; font-weight: 700; margin-top: 8px; }}
.score {{ font-size: 80px; margin-top: 14px; line-height: 1; font-weight: 700; }}
.middle {{ text-align: center; display: flex; flex-direction: column; justify-content: center; }}
.middle .v {{ font-size: 56px; line-height: 1.1; font-weight: 700; }}
.details {{
  margin-top: 12px;
  display: grid;
  grid-template-columns: repeat(6, 1fr);
  gap: 10px;
  border: 1px solid var(--border);
  border-radius: 14px;
  padding: 10px;
  background: #112b58;
}}
.detail {{
  border: 1px solid var(--border);
  border-radius: 12px;
  background: #0c1f46;
  min-height: 100px;
  padding: 10px;
}}
.detail .k {{ text-transform: uppercase; color: var(--muted); font-size: 18px; letter-spacing: 0.06em; }}
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

    document.getElementById('home_team').textContent = choose(row, ['HomeTeamName', 'home_team']);
    document.getElementById('away_team').textContent = choose(row, ['GuestTeamName', 'away_team']);
    document.getElementById('home_score').textContent = choose(row, ['HomeTeamScore', 'home_score']);
    document.getElementById('away_score').textContent = choose(row, ['GuestTeamScore', 'away_score']);
    document.getElementById('period').textContent = choose(row, ['Period', 'period', 'quarter']);
    document.getElementById('clock').textContent = choose(row, ['MainClockTime', 'clock', 'main_clock_time'], '--:--');
    document.getElementById('home_timeouts').textContent = choose(row, ['HomeTimeOutsLeftTotal', 'home_timeouts', 'home_time_outs_left_total']);
    document.getElementById('away_timeouts').textContent = choose(row, ['GuestTimeOutsLeftTotal', 'away_timeouts', 'guest_time_outs_left_total']);
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
    let (path, _) = target.split_once('?').unwrap_or((target, ""));

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

    if path.starts_with("/admin") {
        if method == "GET" && path == "/admin" && !has_valid_session(req, &state).await {
            return http_ok("text/html", admin_login_html());
        }

        if method == "POST" && path == "/admin/login" {
            return login(req, &state).await;
        }

        if !authorized(req, &state).await {
            return http_unauthorized();
        }
    }

    if method == "GET" && path == "/admin" {
        let body = admin_html();
        return http_ok("text/html", &body);
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

async fn authorized(req: &str, state: &WebState) -> bool {
    has_valid_session(req, state).await
}

async fn has_valid_session(req: &str, state: &WebState) -> bool {
    let Some(session_id) = cookie_value(req, "admin_session") else {
        return false;
    };
    state.sessions.read().await.contains(session_id)
}

async fn login(req: &str, state: &WebState) -> String {
    let body_text = req.split("\r\n\r\n").nth(1).unwrap_or("");
    let form = parse_form(body_text);
    let token = form.get("token").map(String::as_str).unwrap_or("");
    let cfg = state.config.read().await;
    if token != cfg.admin_password_hash {
        return http_unauthorized();
    }
    drop(cfg);

    let session_id = Uuid::new_v4().to_string();
    state.sessions.write().await.insert(session_id.clone());
    http_see_other_with_cookie(
        "/admin",
        &format!(
            "admin_session={session_id}; Path=/admin; HttpOnly; SameSite=Lax"
        ),
    )
}

fn cookie_value<'a>(req: &'a str, name: &str) -> Option<&'a str> {
    req.lines()
        .find_map(|line| line.strip_prefix("Cookie: "))
        .and_then(|cookies| {
            cookies
                .split(';')
                .map(str::trim)
                .filter_map(|pair| pair.split_once('='))
                .find_map(|(k, v)| (k == name).then_some(v))
        })
}

fn admin_login_html() -> &'static str {
    r#"<!doctype html>
<html>
<head>
<meta charset='utf-8'/>
<meta name='viewport' content='width=device-width, initial-scale=1'/>
<title>Daktronics Gateway Admin Login</title>
<style>
:root {
  --bg: #081a3d;
  --panel: linear-gradient(180deg, #1b3567 0%, #182f5e 100%);
  --tile: linear-gradient(180deg, #1a3366 0%, #182f5c 100%);
  --border: #3b67a8;
  --text: #deebff;
  --muted: #9fb8df;
}
* { box-sizing: border-box; }
body {
  margin: 0;
  font-family: Inter, Segoe UI, Roboto, Arial, sans-serif;
  color: var(--text);
  background-color: #061632;
  background-image: radial-gradient(circle at top left, #13366f 0%, #081a3d 52%, #061632 100%);
  background-repeat: no-repeat;
  background-size: cover;
  min-height: 100vh;
  display: grid;
  place-items: center;
  padding: 24px;
}
.card {
  width: min(560px, 100%);
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 18px;
  padding: 28px;
}
h1 { margin: 0; font-size: 34px; }
p { color: var(--muted); margin: 8px 0 22px; font-size: 19px; }
label { display: block; color: var(--muted); font-size: 14px; margin: 0 0 8px; }
input {
  width: 100%;
  border-radius: 12px;
  border: 1px solid #456fad;
  background: #0c1f46;
  color: #eaf2ff;
  padding: 12px 14px;
  font-size: 18px;
}
.row { margin-top: 18px; display: flex; gap: 10px; flex-wrap: wrap; }
.btn {
  border: 1px solid #4c7ec3;
  background: linear-gradient(180deg, #3b82d7 0%, #2f6fbf 100%);
  color: #f1f6ff;
  border-radius: 12px;
  padding: 10px 16px;
  font-size: 16px;
  text-decoration: none;
  cursor: pointer;
}
</style>
</head>
<body>
  <main class='card'>
    <h1>Daktronics Gateway Admin</h1>
    <p>Enter your admin token to open configuration controls.</p>
    <form method='post' action='/admin/login'>
      <label for='token'>Admin Token</label>
      <input id='token' type='password' name='token' placeholder='••••••••' required />
      <div class='row'>
        <button class='btn' type='submit'>Open Admin Panel</button>
        <a class='btn' href='/'>Back to Dashboard</a>
      </div>
    </form>
  </main>
</body>
</html>"#
}

fn admin_html() -> String {
    let template = r#"<!doctype html>
<html>
<head>
<meta charset='utf-8'/>
<meta name='viewport' content='width=device-width, initial-scale=1'/>
<title>Daktronics Gateway Admin</title>
<style>
:root {
  --bg: #081a3d;
  --panel: linear-gradient(180deg, #1b3567 0%, #182f5e 100%);
  --tile: linear-gradient(180deg, #1a3366 0%, #182f5c 100%);
  --border: #3b67a8;
  --text: #deebff;
  --muted: #9fb8df;
}
* { box-sizing: border-box; }
body {
  margin: 0;
  font-family: Inter, Segoe UI, Roboto, Arial, sans-serif;
  color: var(--text);
  background-color: #061632;
  background-image: radial-gradient(circle at top left, #13366f 0%, #081a3d 52%, #061632 100%);
  background-repeat: no-repeat;
  background-size: cover;
  min-height: 100vh;
}
.shell { max-width: 1160px; margin: 10px auto; padding: 0 8px 12px; }
.header {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 16px;
  padding: 14px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 14px;
}
.title h1 { margin: 0; font-size: 38px; }
.title p { margin: 6px 0 0; color: var(--muted); font-size: 28px; }
.btns { display: flex; gap: 10px; flex-wrap: wrap; }
.btn {
  border: 1px solid #4c7ec3;
  background: linear-gradient(180deg, #2f62ab 0%, #2a5796 100%);
  color: #e8f1ff;
  border-radius: 12px;
  padding: 10px 14px;
  font-size: 22px;
  text-decoration: none;
  cursor: pointer;
}
.grid {
  margin-top: 14px;
  display: grid;
  grid-template-columns: repeat(4, minmax(210px, 1fr));
  gap: 12px;
}
.card {
  background: var(--tile);
  border: 1px solid var(--border);
  border-radius: 16px;
  padding: 16px;
  min-height: 300px;
}
.card h2 { margin: 0; font-size: 36px; }
.desc { margin: 10px 0 16px; color: var(--muted); font-size: 26px; min-height: 64px; }
label { display: block; margin: 12px 0 6px; color: var(--muted); font-size: 25px; font-weight: 600; }
input, select {
  width: 100%;
  border-radius: 12px;
  border: 1px solid #456fad;
  background: #0c1f46;
  color: #eaf2ff;
  padding: 10px 12px;
  font-size: 28px;
}
.actions { display: flex; gap: 10px; flex-wrap: wrap; margin-top: 18px; }
button.btn { font-size: 28px; }
@media (max-width: 1100px) {
  .title h1 { font-size: 30px; }
  .title p, .btn, button.btn, .desc, label, input, select, .card h2 { font-size: 18px; }
  .grid { grid-template-columns: repeat(2, minmax(220px, 1fr)); }
  .card { min-height: 0; }
}
@media (max-width: 760px) {
  .header { flex-direction: column; align-items: flex-start; }
  .grid { grid-template-columns: 1fr; }
}
</style>
</head>
<body>
  <div class='shell'>
    <section class='header'>
      <div class='title'>
        <h1>Daktronics Gateway Admin</h1>
        <p>Configuration and simulation controls</p>
      </div>
      <div class='btns'>
        <a class='btn' href='/'>Open Dashboard</a>
        <a class='btn' href='/admin/config'>View Status JSON</a>
      </div>
    </section>

    <section class='grid'>
      <article class='card'>
        <h2>Connection settings</h2>
        <p class='desc'>Serial feed details for the connected scoreboard stream.</p>
        <label>Serial Device</label>
        <input value='/dev/ttyUSB0' readonly />
      </article>

      <article class='card'>
        <h2>Sport/controller settings</h2>
        <p class='desc'>Select decoder profile and active sport family.</p>
        <form method='post' action='/admin/controller'>
          <label for='controller_type'>Controller Type</label>
          <select id='controller_type' name='controller_type'>
            <option value='daktronics' selected>all_sport_5000</option>
          </select>
          <div class='actions'>
            <button class='btn' type='submit'>Apply Controller</button>
          </div>
        </form>
        <form method='post' action='/admin/sport'>
          <label for='active_sport'>Sport Type</label>
          <select id='active_sport' name='active_sport'>
            <option value='baseball'>baseball</option>
            <option value='basketball' selected>basketball</option>
            <option value='football'>football</option>
            <option value='soccer'>soccer</option>
            <option value='volleyball'>volleyball</option>
            <option value='wrestling'>wrestling</option>
            <option value='water_polo'>water_polo</option>
          </select>
          <div class='actions'>
            <button class='btn' type='submit'>Apply Sport</button>
          </div>
        </form>
      </article>

      <article class='card'>
        <h2>Publish settings</h2>
        <p class='desc'>Public status URL controls used for outbound updates.</p>
        <label>Public Status Endpoint</label>
        <input value='/status/&lt;uuid&gt;.json' readonly />
        <div class='actions'>
          <form method='post' action='/admin/rotate'>
            <button class='btn' type='submit'>Rotate Public URL</button>
          </form>
        </div>
      </article>

      <article class='card'>
        <h2>Save configuration</h2>
        <p class='desc'>Apply changes to active runtime configuration.</p>
        <div class='actions'>
          <form method='post' action='/admin/controller'>
            <input type='hidden' name='controller_type' value='daktronics' />
            <button class='btn' type='submit'>Save</button>
          </form>
        </div>
      </article>
    </section>
  </div>
</body>
</html>"#;

    template.to_string()
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
    Uuid::new_v4().to_string()
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

fn http_see_other_with_cookie(location: &str, cookie: &str) -> String {
    format!(
        "HTTP/1.1 303 See Other\r\nLocation: {location}\r\nSet-Cookie: {cookie}\r\nContent-Length: 0\r\n\r\n"
    )
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
