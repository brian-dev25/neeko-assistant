use serde::Deserialize;
use std::collections::HashMap;

use crate::config::AppConfig;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

#[allow(dead_code)]
fn get_regional_route(region: &str) -> &str {
    match region.to_lowercase().as_str() {
        "na" | "br" | "lan" | "las" | "la1" | "la2" => "americas",
        "euw" | "eune" | "tr" | "ru" => "europe",
        "kr" | "jp" => "asia",
        _ => "americas",
    }
}

#[allow(dead_code)]
fn normalize_region(region: &str) -> &str {
    match region.to_lowercase().as_str() {
        "las" | "la2" => "las",
        "lan" | "la1" => "la1",
        "na" => "na",
        "br" => "br",
        "euw" => "euw",
        "eune" | "eun" => "eune",
        "kr" => "kr",
        "jp" => "jp",
        "oce" => "oce",
        "tr" => "tr",
        "ru" => "ru",
        _ => region,
    }
}

#[allow(dead_code)]
fn get_queue_name(queue_id: u32) -> &'static str {
    match queue_id {
        420 => "Ranked Solo",
        430 => "Normal Blind",
        440 => "Normal Draft",
        450 => "ARAM",
        900 => "URF",
        1700 => "Arena",
        _ => "Otro",
    }
}

fn format_duration(seconds: u64) -> String {
    let mins = seconds / 60;
    let secs = seconds % 60;
    format!("{:02}:{:02}", mins, secs)
}

fn days_in_month(year: u64, month: u64) -> u64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn is_leap_year(year: u64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn to_epoch_seconds(year: u64, month: u64, day: u64) -> u64 {
    let mut total_days: u64 = 0;
    // Days from years
    for y in 1970..year {
        total_days += if is_leap_year(y) { 366 } else { 365 };
    }
    // Days from months in current year
    for m in 1..month {
        total_days += days_in_month(year, m);
    }
    total_days += day - 1;
    total_days * 86400
}

fn parse_opgg_date(date_str: &str) -> String {
    // ISO 8601: "2026-08-26T14:07:34+09:00"
    let date_time = if let Some(t_pos) = date_str.find('T') {
        &date_str[..t_pos]
    } else {
        return "desconocido".to_string();
    };

    let dt_parts: Vec<&str> = date_time.split('-').collect();
    if dt_parts.len() < 3 {
        return "desconocido".to_string();
    }

    let year: u64 = dt_parts[0].parse().unwrap_or(0);
    let month: u64 = dt_parts[1].parse().unwrap_or(0);
    let day: u64 = dt_parts[2].parse().unwrap_or(0);

    let game_secs = to_epoch_seconds(year, month, day);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let diff = now.saturating_sub(game_secs);

    if diff < 60 {
        "hace un momento".to_string()
    } else if diff < 3600 {
        format!("hace {}m", diff / 60)
    } else if diff < 86400 {
        format!("hace {}h", diff / 3600)
    } else if diff < 604800 {
        format!("hace {}d", diff / 86400)
    } else {
        format!("hace {}sem", diff / 604800)
    }
}

static CHAMPION_CACHE: std::sync::OnceLock<
    tokio::sync::Mutex<Option<HashMap<u32, (String, String)>>>,
> = std::sync::OnceLock::new();

async fn get_champion_map() -> Result<HashMap<u32, (String, String)>, String> {
    let lock = CHAMPION_CACHE.get_or_init(|| tokio::sync::Mutex::new(None));
    let mut cache = lock.lock().await;
    if let Some(ref map) = *cache {
        return Ok(map.clone());
    }

    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let version_resp: Vec<String> = client
        .get("https://ddragon.leagueoflegends.com/api/versions.json")
        .send()
        .await
        .map_err(|e| format!("Error obteniendo versiones: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Error parseando versiones: {}", e))?;

    let version = version_resp
        .first()
        .ok_or("No ddragon version disponible")?;

    let data: serde_json::Value = client
        .get(format!(
            "https://ddragon.leagueoflegends.com/cdn/{}/data/es_MX/champion.json",
            version
        ))
        .send()
        .await
        .map_err(|e| format!("Error obteniendo campeones: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Error parseando campeones: {}", e))?;

    let mut map = HashMap::new();
    if let Some(champs) = data["data"].as_object() {
        for (_name, info) in champs {
            if let (Some(key_str), Some(name)) = (info["key"].as_str(), info["name"].as_str()) {
                if let Ok(id) = key_str.parse::<u32>() {
                    let icon = format!(
                        "https://ddragon.leagueoflegends.com/cdn/{}/img/champion/{}.png",
                        version,
                        info["image"]["full"].as_str().unwrap_or("")
                    );
                    map.insert(id, (name.to_string(), icon));
                }
            }
        }
    }

    *cache = Some(map.clone());
    Ok(map)
}

#[derive(Deserialize)]
struct OpggSummonerSearch {
    data: Vec<OpggSummoner>,
}

#[derive(Deserialize)]
struct OpggSummoner {
    puuid: String,
    game_name: String,
    tagline: String,
    #[serde(default)]
    level: u64,
    #[serde(default)]
    solo_tier_info: Option<OpggTierInfo>,
}

#[derive(Deserialize)]
struct OpggTierInfo {
    tier: Option<String>,
    division: Option<u32>,
    lp: Option<u32>,
}

#[derive(Deserialize)]
struct OpggGamesResponse {
    data: Vec<OpggGame>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OpggGame {
    #[serde(default)]
    game_length_second: u64,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    queue_id: Option<u32>,
    #[serde(default)]
    game_type: Option<String>,
    #[serde(default)]
    participants: Vec<OpggParticipant>,
}

#[derive(Deserialize, Default)]
struct OpggParticipant {
    #[serde(default)]
    champion_id: u32,
    #[serde(default)]
    summoner: Option<OpggParticipantSummoner>,
    #[serde(default)]
    stats: OpggParticipantStats,
}

#[derive(Deserialize, Default)]
struct OpggParticipantSummoner {
    #[serde(default)]
    puuid: String,
}

#[derive(Deserialize, Default)]
struct OpggParticipantStats {
    #[serde(default)]
    kill: u32,
    #[serde(default)]
    death: u32,
    #[serde(default)]
    assist: u32,
    #[serde(default)]
    result: String,
}

#[tauri::command]
pub async fn lol_get_match_history(
    riot_id: String,
    region: String,
    count: Option<i32>,
) -> Result<String, String> {
    let region = normalize_region(&region);
    let count = count.unwrap_or(5).min(20);

    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let parts: Vec<&str> = riot_id.split('#').collect();
    let game_name = parts[0].trim();
    let tag_line = if parts.len() > 1 {
        parts[1].trim().to_string()
    } else {
        region.to_uppercase()
    };

    let search_url = format!(
        "https://lol-api-summoner.op.gg/api/v3/{}/summoners?riot_id={}&hl=es_MX",
        region,
        urlencoding::encode(&format!("{}#{}", game_name, tag_line))
    );

    let search_data: OpggSummonerSearch = client
        .get(&search_url)
        .send()
        .await
        .map_err(|e| format!("Error de red al buscar summoner: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Error parseando respuesta de summoner: {}", e))?;

    let summoner = search_data.data.first().ok_or_else(|| {
        format!(
            "No encontré al jugador {}#{} en {}",
            game_name, tag_line, region
        )
    })?;

    eprintln!(
        "[NEEKO] Found summoner: {}#{}",
        summoner.game_name, summoner.tagline
    );

    let player_puuid = summoner.puuid.clone();

    let games_url = format!(
        "https://lol-api-summoner.op.gg/api/v3/{}/summoners/{}/games?limit={}&game_type=total&hl=es_MX",
        region, player_puuid, count
    );

    let games_resp = client
        .get(&games_url)
        .send()
        .await
        .map_err(|e| format!("Error de red al obtener partidas: {}", e))?;

    let games_text = games_resp
        .text()
        .await
        .map_err(|e| format!("Error leyendo body: {}", e))?;

    let games_response: OpggGamesResponse = serde_json::from_str(&games_text)
        .map_err(|e| format!("Error parseando partidas: {}", e))?;

    let champion_map = get_champion_map().await.unwrap_or_default();

    let mut lines = Vec::new();
    lines.push(format!(
        "📊 {}#{} (Lvl {})",
        summoner.game_name, summoner.tagline, summoner.level
    ));

    for game in games_response.data.iter() {
        let participant = game
            .participants
            .iter()
            .find(|p| p.summoner.as_ref().map(|s| s.puuid.as_str()) == Some(player_puuid.as_str()));
        let p = match participant {
            Some(p) => p,
            None => continue,
        };

        let (champ_name, _champ_icon) = champion_map
            .get(&p.champion_id)
            .cloned()
            .unwrap_or_else(|| (format!("Champ#{}", p.champion_id), String::new()));

        let result_emoji = match p.stats.result.as_str() {
            "WIN" => "🏆",
            _ => "💀",
        };

        let kda_ratio = if p.stats.death == 0 {
            (p.stats.kill + p.stats.assist) as f64
        } else {
            (p.stats.kill + p.stats.assist) as f64 / p.stats.death as f64
        };

        let date_str = parse_opgg_date(&game.created_at);

        lines.push(format!(
            "\n{} {} {} | {}/{}/{} ({:.1}) | {} | {}",
            result_emoji,
            champ_name,
            if p.stats.result == "WIN" {
                "Victoria"
            } else {
                "Derrota"
            },
            p.stats.kill,
            p.stats.death,
            p.stats.assist,
            kda_ratio,
            format_duration(game.game_length_second),
            date_str
        ));
    }

    if lines.len() <= 1 {
        return Ok("No encontré partidas recientes para este jugador 🥺".to_string());
    }

    Ok(lines.join("\n"))
}

#[tauri::command]
pub fn lol_save_config(
    region: Option<String>,
    git_path: Option<String>,
    git_pat: Option<String>,
    riot_id: Option<String>,
    neeko_sprite: Option<String>,
) -> Result<String, String> {
    let mut config = AppConfig::load();
    if let Some(r) = region {
        config.lol_region = r;
    }
    if let Some(p) = git_path {
        config.git_default_path = p;
    }
    if let Some(pat) = git_pat {
        config.git_pat = pat;
    }
    if let Some(id) = riot_id {
        config.riot_id = id;
    }
    if let Some(sprite) = neeko_sprite {
        match sprite.as_str() {
            "NEEKO.png" | "NEEKO-standing-costume.png" | "NEEKO-sitting.png" => {
                config.neeko_sprite = sprite;
            }
            _ => return Err("Sprite de Neeko no valido".to_string()),
        }
    }
    config.save()?;
    Ok("Configuración guardada ✅".to_string())
}

#[tauri::command]
pub fn lol_get_config() -> Result<String, String> {
    let config = AppConfig::load();
    let safe_pat = if config.git_pat.is_empty() {
        String::new()
    } else if config.git_pat.len() > 8 {
        format!(
            "{}...{}",
            &config.git_pat[..4],
            &config.git_pat[config.git_pat.len() - 4..]
        )
    } else {
        "****".to_string()
    };
    Ok(serde_json::json!({
        "git_pat_masked": safe_pat,
        "git_default_path": config.git_default_path,
        "ffmpeg_path": config.ffmpeg_path,
        "ffprobe_path": config.ffprobe_path,
        "neeko_sprite": match config.neeko_sprite.as_str() {
            "NEEKO-standing-costume.png" => "NEEKO-standing-costume.png",
            "NEEKO-sitting.png" => "NEEKO-sitting.png",
            _ => "NEEKO.png",
        },
        "lol_region": config.lol_region,
        "riot_id": config.riot_id,
    })
    .to_string())
}

fn tier_name(tier: &str) -> &str {
    match tier.to_uppercase().as_str() {
        "IRON" => "Hierro",
        "BRONZE" => "Bronce",
        "SILVER" => "Plata",
        "GOLD" => "Oro",
        "PLATINUM" => "Platino",
        "EMERALD" => "Esmeralda",
        "DIAMOND" => "Diamante",
        "MASTER" => "Maestro",
        "GRANDMASTER" => "Gran Maestro",
        "CHALLENGER" => "Retador",
        _ => tier,
    }
}

#[tauri::command]
pub async fn lol_get_rank(riot_id: String, region: String) -> Result<String, String> {
    let region = normalize_region(&region);

    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let parts: Vec<&str> = riot_id.split('#').collect();
    let game_name = parts[0].trim();
    let tag_line = if parts.len() > 1 {
        parts[1].trim().to_string()
    } else {
        region.to_uppercase()
    };

    let search_url = format!(
        "https://lol-api-summoner.op.gg/api/v3/{}/summoners?riot_id={}&hl=es_MX",
        region,
        urlencoding::encode(&format!("{}#{}", game_name, tag_line))
    );

    let search_data: OpggSummonerSearch = client
        .get(&search_url)
        .send()
        .await
        .map_err(|e| format!("Error de red al buscar summoner: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Error parseando respuesta de summoner: {}", e))?;

    let summoner = search_data.data.first().ok_or_else(|| {
        format!(
            "No encontré al jugador {}#{} en {}",
            game_name, tag_line, region
        )
    })?;

    let tier_info = summoner.solo_tier_info.as_ref().ok_or_else(|| {
        format!(
            "{}#{} no tiene clasificación activa este season",
            summoner.game_name, summoner.tagline
        )
    })?;

    let tier = tier_info.tier.as_deref().unwrap_or("UNRANKED");
    let division = tier_info.division.unwrap_or(0);
    let lp = tier_info.lp.unwrap_or(0);

    let rank_str = if tier == "MASTER" || tier == "GRANDMASTER" || tier == "CHALLENGER" {
        format!("{} {} LP", tier_name(tier), lp)
    } else {
        format!("{} {} - {} LP", tier_name(tier), division, lp)
    };

    Ok(format!(
        "🏆 {}#{} está en {} este season",
        summoner.game_name, summoner.tagline, rank_str
    ))
}
