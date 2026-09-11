//! Windows key state polling lives with the running addon, including when its UI is closed.
use super::*;

fn parse(value: &str) -> Result<Vec<i32>, String> {
    if value.is_empty() {
        return Ok(vec![]);
    }
    let mut keys = Vec::new();
    for part in value.split('+') {
        let key =
            match part {
                "Ctrl" => 0x11,
                "Shift" => 0x10,
                "Alt" => 0x12,
                "Space" => 0x20,
                s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphanumeric() => {
                    s.as_bytes()[0].to_ascii_uppercase() as i32
                }
                s if s.starts_with('F') => s[1..]
                    .parse::<i32>()
                    .ok()
                    .filter(|n| (1..=24).contains(n))
                    .map(|n| 0x6f + n)
                    .ok_or("Tecla de función inválida")?,
                _ => return Err(
                    "Atajo inválido. Usá letras, números, Space o F1–F24 con Ctrl, Shift o Alt."
                        .into(),
                ),
            };
        if keys.contains(&key) {
            return Err("Tecla repetida en el atajo".into());
        }
        keys.push(key);
    }
    if keys
        .iter()
        .filter(|k| ![0x10, 0x11, 0x12].contains(k))
        .count()
        != 1
    {
        return Err("El atajo necesita una tecla principal".into());
    }
    keys.sort();
    Ok(keys)
}

pub(super) fn validate_keys(talk: &str, mute: &str) -> Result<(), String> {
    let a = parse(talk)?;
    let b = parse(mute)?;
    if a.is_empty() {
        return Err("Elegí una tecla para pulsar para hablar".into());
    }
    if a == b {
        return Err("Hablar y silenciar necesitan atajos distintos".into());
    }
    Ok(())
}

fn down(keys: &[i32]) -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
        // Only the high bit is reliable. No hooks, captured text or key history.
        !keys.is_empty()
            && keys.iter().all(|k| unsafe { GetAsyncKeyState(*k) < 0 })
            && [0x10, 0x11, 0x12]
                .iter()
                .all(|k| keys.contains(k) || unsafe { GetAsyncKeyState(*k) >= 0 })
    }
    #[cfg(not(windows))]
    {
        let _ = keys;
        false
    }
}

#[derive(Default)]
struct Edge {
    armed: bool,
    previous: bool,
}
impl Edge {
    fn tick(&mut self, down: bool) -> bool {
        if !down {
            self.armed = true;
        }
        let pressed = self.armed && down && !self.previous;
        self.previous = down;
        pressed
    }
}

pub(super) async fn run(
    events: Arc<Events>,
    blocked: Arc<AtomicBool>,
    active: neeko_micyou_core::tcp_server::SharedActiveConnection,
    stats: Arc<neeko_micyou_core::stats::NetworkStats>,
) {
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(16));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut binding = String::new();
    let mut mute_edge = Edge::default();
    let mut talk_edge = Edge::default();
    loop {
        tick.tick().await;
        let s = events.status.lock().unwrap().settings.clone();
        let current = format!("{}|{}|{}", s.input_mode, s.talk_key, s.mute_key);
        if binding != current {
            binding = current;
            mute_edge = Edge::default();
            talk_edge = Edge::default();
        }
        let talk_down = down(&parse(&s.talk_key).unwrap_or_default());
        talk_edge.tick(talk_down);
        let pressed = s.input_mode == "ptt" && talk_edge.armed && talk_down;
        blocked.store(s.input_mode == "ptt" && !pressed, Ordering::Release);
        let changed = events.status.lock().unwrap().talk_pressed != pressed;
        if changed {
            events.update(|s| s.talk_pressed = pressed);
        }
        if mute_edge.tick(down(&parse(&s.mute_key).unwrap_or_default())) {
            if let Err(e) = apply_mute(&events, &active, &stats, !s.muted).await {
                events.update(|s| s.message = e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bindings_are_validated_and_compared_canonically() {
        assert!(validate_keys("Space", "Ctrl+Shift+M").is_ok());
        assert!(validate_keys("X", "").is_ok());
        for (a, b) in [
            ("Ctrl+X", "X+Ctrl"),
            ("", "X"),
            ("F25", "X"),
            ("Ctrl", "X"),
            ("X+Y", "M"),
        ] {
            assert!(validate_keys(a, b).is_err());
        }
    }
    #[test]
    fn held_key_does_not_toggle_until_released_and_repeat_is_ignored() {
        let mut e = Edge::default();
        assert!(!e.tick(true));
        assert!(!e.tick(false));
        assert!(e.tick(true));
        assert!(!e.tick(true));
        assert!(!e.tick(false));
        assert!(e.tick(true));
    }
}
