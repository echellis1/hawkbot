use std::{env, fs, path::Path};

use daktronics_allsport_5000::{
    rtd_state::RTDStateFieldError,
    sports::{basketball::BasketballSport, Sport},
    RTDState,
};
use serde::Serialize;
use tokio_serial::SerialPortBuilderExt;

#[cfg(unix)]
const DEFAULT_TTY: &str = "/dev/ttyUSB0";
#[cfg(windows)]
const DEFAULT_TTY: &str = "COM1";

const DEFAULT_OUTPUT_PATH: &str = "./basketball-live.json";

#[derive(Serialize)]
struct BasketballSnapshot {
    #[serde(rename = "MainClockTime")]
    main_clock_time: Option<String>,
    #[serde(rename = "MainClockTime2")]
    main_clock_time_2: Option<String>,
    #[serde(rename = "MainClockTimeOutTod")]
    main_clock_time_out_tod: Option<String>,
    #[serde(rename = "MainClockTimeOutTod2")]
    main_clock_time_out_tod_2: Option<String>,
    #[serde(rename = "MainClockIsZero")]
    main_clock_is_zero: Option<bool>,
    #[serde(rename = "MainClockStopped")]
    main_clock_stopped: Option<bool>,
    #[serde(rename = "MainClockTimeOutHorn")]
    main_clock_time_out_horn: Option<bool>,
    #[serde(rename = "MainClockHorn")]
    main_clock_horn: Option<bool>,
    #[serde(rename = "TimeOutHorn")]
    time_out_horn: Option<bool>,
    #[serde(rename = "TimeOutTime")]
    time_out_time: Option<String>,
    #[serde(rename = "TimeOfDay")]
    time_of_day: Option<String>,
    #[serde(rename = "HomeTeamName")]
    home_team_name: Option<String>,
    #[serde(rename = "GuestTeamName")]
    guest_team_name: Option<String>,
    #[serde(rename = "HomeTeamScore")]
    home_team_score: Option<i32>,
    #[serde(rename = "GuestTeamScore")]
    guest_team_score: Option<i32>,
    #[serde(rename = "HomeTimeOutsLeftFull")]
    home_time_outs_left_full: Option<i32>,
    #[serde(rename = "HomeTimeOutsLeftPartial")]
    home_time_outs_left_partial: Option<i32>,
    #[serde(rename = "HomeTimeOutsLeftTotal")]
    home_time_outs_left_total: Option<i32>,
    #[serde(rename = "GuestTimeOutsLeftFull")]
    guest_time_outs_left_full: Option<i32>,
    #[serde(rename = "GuestTimeOutsLeftPartial")]
    guest_time_outs_left_partial: Option<i32>,
    #[serde(rename = "GuestTimeOutsLeftTotal")]
    guest_time_outs_left_total: Option<i32>,
    #[serde(rename = "HomeTimeOutIndicator")]
    home_time_out_indicator: Option<bool>,
    #[serde(rename = "HomeTimeOutText")]
    home_time_out_text: Option<String>,
    #[serde(rename = "GuestTimeOutIndicator")]
    guest_time_out_indicator: Option<bool>,
    #[serde(rename = "GuestTimeOutText")]
    guest_time_out_text: Option<String>,
    #[serde(rename = "Period")]
    period: Option<i32>,
    #[serde(rename = "InternalRelay")]
    internal_relay: Option<bool>,
    #[serde(rename = "AdPanelCaptionPower")]
    ad_panel_caption_power: Option<bool>,
    #[serde(rename = "AdPanelCaptionNum1")]
    ad_panel_caption_num_1: Option<bool>,
    #[serde(rename = "AdPanelCaptionNum2")]
    ad_panel_caption_num_2: Option<bool>,
    #[serde(rename = "ShotClockTime")]
    shot_clock_time: Option<String>,
    #[serde(rename = "ShotClockHorn")]
    shot_clock_horn: Option<bool>,
    #[serde(rename = "HomePossessionIndicator")]
    home_possession_indicator: Option<bool>,
    #[serde(rename = "HomePossessionText")]
    home_possession_text: Option<String>,
    #[serde(rename = "GuestPossessionIndicator")]
    guest_possession_indicator: Option<bool>,
    #[serde(rename = "GuestPossessionText")]
    guest_possession_text: Option<String>,
    #[serde(rename = "Home1On1BonusIndicator")]
    home_1_on_1_bonus_indicator: Option<bool>,
    #[serde(rename = "Home2ShotBonusIndicator")]
    home_2_shot_bonus_indicator: Option<bool>,
    #[serde(rename = "HomeBonusText")]
    home_bonus_text: Option<String>,
    #[serde(rename = "Guest1On1BonusIndicator")]
    guest_1_on_1_bonus_indicator: Option<bool>,
    #[serde(rename = "Guest2ShotBonusIndicator")]
    guest_2_shot_bonus_indicator: Option<bool>,
    #[serde(rename = "GuestBonusText")]
    guest_bonus_text: Option<String>,
    #[serde(rename = "HomeTeamFouls")]
    home_team_fouls: Option<i32>,
    #[serde(rename = "GuestTeamFouls")]
    guest_team_fouls: Option<i32>,
    #[serde(rename = "HomePlayerFoulPoints")]
    home_player_foul_points: Option<i32>,
    #[serde(rename = "GuestPlayerFoulPoints")]
    guest_player_foul_points: Option<i32>,
    #[serde(rename = "PlayerFoul")]
    player_foul: Option<i32>,
    #[serde(rename = "PlayerFoulPlayer")]
    player_foul_player: Option<String>,
    #[serde(rename = "PlayerFoulFoul")]
    player_foul_foul: Option<String>,
    #[serde(rename = "PlayerFoulPoints")]
    player_foul_points: Option<String>,
    #[serde(rename = "HomeScorePeriod1")]
    home_score_period_1: Option<i32>,
    #[serde(rename = "HomeScorePeriod2")]
    home_score_period_2: Option<i32>,
    #[serde(rename = "HomeScorePeriod3")]
    home_score_period_3: Option<i32>,
    #[serde(rename = "HomeScorePeriod4")]
    home_score_period_4: Option<i32>,
    #[serde(rename = "HomeScorePeriod5")]
    home_score_period_5: Option<i32>,
    #[serde(rename = "HomeScorePeriod7")]
    home_score_period_7: Option<i32>,
    #[serde(rename = "HomeScorePeriod8")]
    home_score_period_8: Option<i32>,
    #[serde(rename = "HomeScorePeriod9")]
    home_score_period_9: Option<i32>,
    #[serde(rename = "HomeScoreCurrentPeriod")]
    home_score_current_period: Option<i32>,
    #[serde(rename = "GuestScorePeriod1")]
    guest_score_period_1: Option<i32>,
    #[serde(rename = "GuestScorePeriod2")]
    guest_score_period_2: Option<i32>,
    #[serde(rename = "GuestScorePeriod3")]
    guest_score_period_3: Option<i32>,
    #[serde(rename = "GuestScorePeriod4")]
    guest_score_period_4: Option<i32>,
    #[serde(rename = "GuestScorePeriod5")]
    guest_score_period_5: Option<i32>,
    #[serde(rename = "GuestScorePeriod6")]
    guest_score_period_6: Option<i32>,
    #[serde(rename = "GuestScorePeriod7")]
    guest_score_period_7: Option<i32>,
    #[serde(rename = "GuestScorePeriod8")]
    guest_score_period_8: Option<i32>,
    #[serde(rename = "GuestScorePeriod9")]
    guest_score_period_9: Option<i32>,
}

impl BasketballSnapshot {
    fn from_sport(
        sport: &BasketballSport<
            impl daktronics_allsport_5000::rtd_state::data_source::RTDStateDataSource,
        >,
    ) -> Self {
        Self {
            main_clock_time: optional_str(sport.main_clock_time()),
            main_clock_time_2: optional_str(sport.main_clock_time_2()),
            main_clock_time_out_tod: optional_str(sport.main_clock_time_out_tod()),
            main_clock_time_out_tod_2: optional_str(sport.main_clock_time_out_tod_2()),
            main_clock_is_zero: optional(sport.main_clock_is_zero()),
            main_clock_stopped: optional(sport.main_clock_stopped()),
            main_clock_time_out_horn: optional(sport.main_clock_time_out_horn()),
            main_clock_horn: optional(sport.main_clock_horn()),
            time_out_horn: optional(sport.time_out_horn()),
            time_out_time: optional_str(sport.time_out_time()),
            time_of_day: optional_str(sport.time_of_day()),
            home_team_name: optional_str(sport.home_team_name()),
            guest_team_name: optional_str(sport.guest_team_name()),
            home_team_score: optional(sport.home_team_score()),
            guest_team_score: optional(sport.guest_team_score()),
            home_time_outs_left_full: optional(sport.home_time_outs_left_full()),
            home_time_outs_left_partial: optional(sport.home_time_outs_left_partial()),
            home_time_outs_left_total: optional(sport.home_time_outs_left_total()),
            guest_time_outs_left_full: optional(sport.guest_time_outs_left_full()),
            guest_time_outs_left_partial: optional(sport.guest_time_outs_left_partial()),
            guest_time_outs_left_total: optional(sport.guest_time_outs_left_total()),
            home_time_out_indicator: optional(sport.home_time_out_indicator()),
            home_time_out_text: optional_str(sport.home_time_out_text()),
            guest_time_out_indicator: optional(sport.guest_time_out_indicator()),
            guest_time_out_text: optional_str(sport.guest_time_out_text()),
            period: optional(sport.period()),
            internal_relay: optional(sport.internal_relay()),
            ad_panel_caption_power: optional(sport.ad_panel_caption_power()),
            ad_panel_caption_num_1: optional(sport.ad_panel_caption_num_1()),
            ad_panel_caption_num_2: optional(sport.ad_panel_caption_num_2()),
            shot_clock_time: optional_str(sport.shot_clock_time()),
            shot_clock_horn: optional(sport.shot_clock_horn()),
            home_possession_indicator: optional(sport.home_possession_indicator()),
            home_possession_text: optional_str(sport.home_possession_text()),
            guest_possession_indicator: optional(sport.guest_possession_indicator()),
            guest_possession_text: optional_str(sport.guest_possession_text()),
            home_1_on_1_bonus_indicator: optional(sport.home_1_on_1_bonus_indicator()),
            home_2_shot_bonus_indicator: optional(sport.home_2_shot_bonus_indicator()),
            home_bonus_text: optional_str(sport.home_bonus_text()),
            guest_1_on_1_bonus_indicator: optional(sport.guest_1_on_1_bonus_indicator()),
            guest_2_shot_bonus_indicator: optional(sport.guest_2_shot_bonus_indicator()),
            guest_bonus_text: optional_str(sport.guest_bonus_text()),
            home_team_fouls: optional(sport.home_team_fouls()),
            guest_team_fouls: optional(sport.guest_team_fouls()),
            home_player_foul_points: optional(sport.home_player_foul_points()),
            guest_player_foul_points: optional(sport.guest_player_foul_points()),
            player_foul: optional(sport.player_foul()),
            player_foul_player: optional_str(sport.player_foul_player()),
            player_foul_foul: optional_str(sport.player_foul_foul()),
            player_foul_points: optional_str(sport.player_foul_points()),
            home_score_period_1: optional(sport.home_score_period_1()),
            home_score_period_2: optional(sport.home_score_period_2()),
            home_score_period_3: optional(sport.home_score_period_3()),
            home_score_period_4: optional(sport.home_score_period_4()),
            home_score_period_5: optional(sport.home_score_period_5()),
            home_score_period_7: optional(sport.home_score_period_7()),
            home_score_period_8: optional(sport.home_score_period_8()),
            home_score_period_9: optional(sport.home_score_period_9()),
            home_score_current_period: optional(sport.home_score_current_period()),
            guest_score_period_1: optional(sport.guest_score_period_1()),
            guest_score_period_2: optional(sport.guest_score_period_2()),
            guest_score_period_3: optional(sport.guest_score_period_3()),
            guest_score_period_4: optional(sport.guest_score_period_4()),
            guest_score_period_5: optional(sport.guest_score_period_5()),
            guest_score_period_6: optional(sport.guest_score_period_6()),
            guest_score_period_7: optional(sport.guest_score_period_7()),
            guest_score_period_8: optional(sport.guest_score_period_8()),
            guest_score_period_9: optional(sport.guest_score_period_9()),
        }
    }
}

fn optional<T>(value: Result<T, RTDStateFieldError>) -> Option<T> {
    match value {
        Ok(v) => Some(v),
        Err(RTDStateFieldError::NoData) => None,
        Err(_) => None,
    }
}

fn optional_str(value: Result<&str, RTDStateFieldError>) -> Option<String> {
    optional(value.map(str::to_string))
}

/// Main function to dump basketball data as JSON.
///
/// This function initializes a serial port connection to the AllSport 5000
/// controller, reads the basketball game data, and writes the latest snapshot
/// into a JSON file each update.
#[tokio::main]
async fn main() -> tokio_serial::Result<()> {
    // Get the command line arguments and determine the TTY path.
    let mut args = env::args();
    let tty_path = args.nth(1).unwrap_or_else(|| DEFAULT_TTY.into());
    let output_path = args.next().unwrap_or_else(|| DEFAULT_OUTPUT_PATH.into());

    // Create the serial port. Note the baud rate and parity.
    // allow because cargo gets suspicious on Windows
    #[allow(unused_mut)]
    let mut port = tokio_serial::new(tty_path, 19200)
        .parity(tokio_serial::Parity::None)
        .open_native_async()?;

    #[cfg(unix)]
    port.set_exclusive(false)
        .expect("Unable to set serial port exclusive to false");

    // Initialize the BasketballSport with the RTDState from the serial stream.
    let mut sport = BasketballSport::new(RTDState::from_serial_stream(port, true).unwrap());

    // Continuously update and write the basketball game data as JSON.
    let mut update_result = Ok(false);
    while let Ok(_) = update_result {
        let snapshot = serde_json::to_string_pretty(&BasketballSnapshot::from_sport(&sport))
            .expect("couldn't read data");
        persist_snapshot(&output_path, &snapshot).expect("couldn't write snapshot");
        println!("Updated {}", output_path);

        update_result = sport.rtd_state().update_async().await;
    }
    if let Err(e) = update_result {
        eprintln!("{:?}", e);
    }
    Ok(())
}

fn persist_snapshot(output_path: &str, snapshot: &str) -> std::io::Result<()> {
    let path = Path::new(output_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, snapshot)
}
