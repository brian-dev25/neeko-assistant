use super::*;
use std::collections::HashMap;
use std::time::Instant;

pub(super) fn lock(app: &AppHandle, locked: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(OVERLAY) {
        window
            .set_ignore_cursor_events(locked)
            .map_err(|e| e.to_string())?;
    }
    update(app, None, |s| s.locked = locked);
    Ok(())
}

static HOTKEYS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(super) fn ensure_hotkeys(app: &AppHandle) {
    use std::sync::atomic::Ordering;
    if HOTKEYS.swap(true,Ordering::SeqCst) {return;}
    let app=app.clone();
    tokio::spawn(async move {
        let mut held=[false;6];
        while enabled() {
            #[cfg(windows)] {
                use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
                for (index,key) in [0x75,0x76,0x77,0x78,0x79,0x7a].iter().enumerate() {
                    let pressed=unsafe{GetAsyncKeyState(0x11)<0&&GetAsyncKeyState(0x12)<0&&GetAsyncKeyState(*key)<0};
                    if pressed&&!held[index] {
                        let action=["start","once","pause","stop","lock","toggle-overlay"][index];
                        let _=app.emit_to("main","screen-translate:shortcut",action);
                    }
                    held[index]=pressed;
                }
            }
            tokio::time::sleep(Duration::from_millis(80)).await;
        }
        HOTKEYS.store(false,Ordering::SeqCst);
    });
}

fn corrected(text: &str, rules: &[Rule], whole_words: bool) -> String {
    rules.iter().fold(text.to_owned(), |text, rule| {
        if !whole_words { return text.replace(&rule.from, &rule.to); }
        let mut result=String::new(); let mut previous=0;
        for (start,matched) in text.match_indices(&rule.from) {
            let end=start+matched.len();
            let word=|c:char|c.is_alphanumeric()||c=='_';
            if text[..start].chars().next_back().is_some_and(word) || text[end..].chars().next().is_some_and(word) {continue;}
            result.push_str(&text[previous..start]);result.push_str(&rule.to);previous=end;
        }
        result.push_str(&text[previous..]);result
    })
}

pub(super) fn merge_lines(mut lines: Vec<Block>) -> Vec<Block> {
    lines.sort_by(|a,b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    let mut blocks:Vec<Block>=vec![];
    let mut last_heights:Vec<f64>=vec![];
    for line in lines {
        let candidate=blocks.iter().enumerate().filter_map(|(i,previous)| {
            let overlap=(previous.x+previous.width).min(line.x+line.width)-previous.x.max(line.x);
            let gap=line.y-(previous.y+previous.height);
            let height=last_heights[i];
            let end=previous.text.trim_end().chars().last().unwrap_or(' ');
            (gap>=-1. && gap<height*0.7 && overlap>line.width.min(previous.width)*0.5
                && line.height/height>0.75 && line.height/height<1.3
                && !['.', '?', '?', '?', '!', '?', ':'].contains(&end))
                .then_some((i,gap))
        }).min_by(|a,b|a.1.total_cmp(&b.1)).map(|v|v.0);
        if let Some(i)=candidate {
            let previous=&mut blocks[i];
            let right=(previous.x+previous.width).max(line.x+line.width);
            previous.x=previous.x.min(line.x);previous.width=right-previous.x;
            previous.height=line.y+line.height-previous.y;
            previous.text.push('\n');previous.text.push_str(&line.text);last_heights[i]=line.height;
        } else {last_heights.push(line.height);blocks.push(line);}
    }
    blocks
}

struct Cache { entries: VecDeque<(String,String)> }
impl Cache {
    fn path()->PathBuf {config_path().with_file_name("screen-translate-cache.json")}
    fn load(enabled:bool)->Self {
        let mut cache=Self{entries:VecDeque::new()};
        if enabled {
            use std::io::Read;
            let mut bytes=vec![];
            if let Ok(file)=std::fs::File::open(Self::path()) {
                if file.take(2_100_001).read_to_end(&mut bytes).is_ok() && bytes.len()<=2_100_000 {
                    if let Ok(entries)=serde_json::from_slice::<Vec<(String,String)>>(&bytes) {
                        for (key,value) in entries.into_iter().take(256) {
                            if key.len()<=48000 && !value.trim().is_empty() && value.len()<=48000 {cache.put(key,value);}
                        }
                    }
                }
            }
        }
        cache
    }
    fn save(&self,enabled:bool)->Result<(),String> {
        if !enabled {return Ok(());}
        let path=Self::path();std::fs::create_dir_all(path.parent().unwrap()).map_err(|e|e.to_string())?;
        let temporary=path.with_extension("json.tmp");
        std::fs::write(&temporary,serde_json::to_vec(&self.entries).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        std::fs::rename(temporary,path).map_err(|e|e.to_string())
    }
    fn get(&mut self,key:&str)->Option<String> {
        let index=self.entries.iter().position(|(k,_)|k==key)?;
        let entry=self.entries.remove(index)?;let result=entry.1.clone();self.entries.push_back(entry);Some(result)
    }
    fn put(&mut self,key:String,text:String) {
        if let Some(i)=self.entries.iter().position(|(k,_)|k==&key) {self.entries.remove(i);}
        self.entries.push_back((key,text));
        while self.entries.len()>256 || self.entries.iter().map(|(k,v)|k.len()+v.len()).sum::<usize>()>2_000_000 {self.entries.pop_front();}
    }
}

pub(super) fn clear_cache()->Result<(),String> {
    match std::fs::remove_file(Cache::path()) {Ok(())=>Ok(()),Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(()),Err(e)=>Err(e.to_string())}
}

fn prepared_text(text:&str,settings:&Settings)->String {
    if settings.join_lines {normalized(text)} else {text.trim().to_owned()}
}
fn cache_key(text:&str,settings:&Settings,source:&str)->String {
    format!("v3\0{}\0{}\0{}\0{}",settings.provider,source,settings.target,text)
}
fn local_translation(text:&str,settings:&Settings)->Option<String> {
    settings.glossary.iter().find(|r|normalized(&r.from)==normalized(text)).map(|r|r.to.clone())
        .or_else(|| (settings.provider=="db").then(||text.to_owned()))
}

fn place(app: &AppHandle, settings: &Settings, frames: &[Frame]) -> Result<(), String> {
    if frames.is_empty() {
        return Ok(());
    }
    let left = frames.iter().map(|f| f.region.x).min().unwrap_or(0);
    let top = frames.iter().map(|f| f.region.y).min().unwrap_or(0);
    let right = frames
        .iter()
        .map(|f| i64::from(f.region.x) + i64::from(f.region.width))
        .max()
        .unwrap_or(660);
    let bottom = frames
        .iter()
        .map(|f| i64::from(f.region.y) + i64::from(f.region.height))
        .max()
        .unwrap_or(240);
    let region = Region {
        x: left,
        y: top,
        width: (right - i64::from(left)) as u32,
        height: (bottom - i64::from(top)) as u32,
    };
    if snapshot().overlay_region.as_ref() != Some(&region) {
        if let Some(window) = app.get_webview_window(OVERLAY) {
            if settings.mode == "over" {
                window
                    .set_min_size(Some(tauri::PhysicalSize::new(20u32, 20u32)))
                    .map_err(|e| e.to_string())?;
                window
                    .set_position(tauri::PhysicalPosition::new(left, top))
                    .map_err(|e| e.to_string())?;
                window
                    .set_size(tauri::PhysicalSize::new(region.width, region.height))
                    .map_err(|e| e.to_string())?;
            } else {
                let mut panel_width = region.width.clamp(300,800);
                let mut panel_height = 200u32;
                let mut panel_x = left;
                let mut panel_y = (bottom + 8) as i32;
                if let Some(monitor)=app.monitor_from_point(f64::from(left)+f64::from(region.width)/2.,f64::from(top)+f64::from(region.height)/2.)
                    .map_err(|e|e.to_string())?.or(app.primary_monitor().map_err(|e|e.to_string())?) {
                    let area=monitor.work_area();
                    panel_width=panel_width.min(area.size.width);panel_height=panel_height.min(area.size.height);
                    panel_x=panel_x.clamp(area.position.x,area.position.x+area.size.width as i32-panel_width as i32);
                    if i64::from(panel_y)+i64::from(panel_height)>i64::from(area.position.y)+i64::from(area.size.height) {panel_y=top-panel_height as i32-8;}
                    panel_y=panel_y.clamp(area.position.y,area.position.y+area.size.height as i32-panel_height as i32);
                    window.set_min_size(Some(tauri::PhysicalSize::new(panel_width.min(300),panel_height.min(100)))).map_err(|e|e.to_string())?;
                }
                window
                    .set_position(tauri::PhysicalPosition::new(panel_x, panel_y))
                    .map_err(|e| e.to_string())?;
                window
                    .set_size(tauri::PhysicalSize::new(panel_width, panel_height))
                    .map_err(|e| e.to_string())?;
            }
        }
        update(app, None, |s| s.overlay_region = Some(region));
    }
    Ok(())
}

async fn translated(
    app: &AppHandle,
    generation: u64,
    client: &reqwest::Client,
    cache: &mut Cache,
    papago: &mut papago::Papago,
    text: &str,
    settings: &Settings,
    source: &str,
) -> Result<String, String> {
    let text=prepared_text(text,settings);
    if let Some(result)=local_translation(&text,settings) {return Ok(result);}
    let key=cache_key(&text,settings,source);
    if let Some(result) = cache.get(&key) {
        update(app, Some(generation), |s| s.cache_hits += 1);
        return Ok(result);
    }
    update(app, Some(generation), |s| s.requests += 1);
    let result = if settings.provider == "papago" {
        papago.translate(client, &text, source, &settings.target).await?

    } else {
        google::translate(client, &text, source, &settings.target).await?
    };
    cache.put(key, result.clone());
    Ok(result)
}

fn split_batch(text: &str, count: usize) -> Option<Vec<String>> {
    let mut remaining=text.trim();let mut results=vec![];
    for index in 0..count {
        let marker=format!("[[NEEKO{index}]]");
        remaining=remaining.strip_prefix(&marker)?.trim_start();
        let next=if index+1==count {"[[NEEKO_END]]".to_owned()} else {format!("[[NEEKO{}]]",index+1)};
        let end=remaining.find(&next)?;
        let value=remaining[..end].trim();
        if value.is_empty() || value.contains("[[NEEKO") {return None;}
        results.push(value.to_owned());remaining=&remaining[end..];
    }
    (remaining=="[[NEEKO_END]]").then_some(results)
}

async fn translate_blocks(app:&AppHandle,generation:u64,client:&reqwest::Client,cache:&mut Cache,
    papago:&mut papago::Papago,blocks:&mut [Block],settings:&Settings,source:&str)->Result<(),String> {
    let mut pending:Vec<(String,Vec<usize>)>=vec![];
    for (index,block) in blocks.iter_mut().enumerate() {
        let text=prepared_text(&block.text,settings);
        if text.is_empty() {continue;}
        if let Some(value)=local_translation(&text,settings) {block.translation=value;continue;}
        if let Some(value)=cache.get(&cache_key(&text,settings,source)) {
            block.translation=value;update(app,Some(generation),|s|s.cache_hits+=1);continue;
        }
        if let Some((_,indices))=pending.iter_mut().find(|(t,_)|t==&text) {indices.push(index);}
        else {pending.push((text,vec![index]));}
    }
    let mut start=0;
    while start<pending.len() {
        let mut end=start;let mut payload=String::new();
        if settings.batch_translation {
            while end<pending.len() && end-start<20 && !pending[end].0.contains("[[NEEKO") {
                let piece=format!("[[NEEKO{}]]\n{}\n",end-start,pending[end].0);
                if payload.chars().count()+piece.chars().count()>3800 {break;}
                payload.push_str(&piece);end+=1;
            }
        }
        let count=end-start;
        let batch=if count>1 {
            payload.push_str("[[NEEKO_END]]");
            update(app,Some(generation),|s|s.requests+=1);
            let response=if settings.provider=="papago" {papago.translate(client,&payload,source,&settings.target).await?}
                else {google::translate(client,&payload,source,&settings.target).await?};
            split_batch(&response,count)
        } else {None};
        if let Some(values)=batch {
            for (offset,value) in values.into_iter().enumerate() {
                let (text,indices)=&pending[start+offset];
                cache.put(cache_key(text,settings,source),value.clone());
                for index in indices {blocks[*index].translation=value.clone();}
            }
            start=end;
        } else {
            // If separators changed, resolve each block independently; never guess positions.
            let stop=end.max(start+1);
            for (text,indices) in &pending[start..stop] {
                let value=translated(app,generation,client,cache,papago,text,settings,source).await?;
                for index in indices {blocks[*index].translation=value.clone();}
            }
            start=stop;
        }
    }
    Ok(())
}

pub(super) async fn run(
    app: &AppHandle,
    generation: u64,
    helper: &mut Helper,
    settings: &Settings,
    once: bool,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;
    let source = snapshot()
        .languages
        .iter()
        .find(|l| l.engine == settings.engine && l.code == settings.source)
        .map(|l| {
            if l.translation_code.is_empty() {
                l.code.clone()
            } else {
                l.translation_code.clone()
            }
        })
        .unwrap_or(settings.source.clone());
    let clipboard = settings.input_mode == "clipboard";
    let source = if !settings.translation_source.is_empty() { settings.translation_source.clone() }
        else if clipboard { "auto".to_owned() } else { source };
    let mut regions = if clipboard { vec![Region { x: 0, y: 0, width: 660, height: 240 }] }
        else { vec![settings.region.clone().ok_or("Selecciona una region")?] };
    if !clipboard { regions.extend(settings.regions.clone()); }
    let mut previous: HashMap<usize, Frame> = HashMap::new();
    let mut cache = Cache::load(settings.persistent_cache);
    let mut papago = papago::Papago::default();
    let mut last = String::new();
    let mut stability = stability::Stability::default();
    let mut last_nonempty = Instant::now();
    if settings.mode == "over" {
        lock(app, true)?;
    } else {
        lock(app, false)?;
    }
    loop {
        if snapshot().paused {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        }
        let cycle_started = Instant::now();
        let mut frames = vec![];
        for (index, region) in regions.iter().enumerate() {
            let mut local = settings.clone();
            local.region = Some(region.clone());
            local.follow_mouse = settings.follow_mouse && index == 0;
            let frame = helper.capture(&local).await?;
            if frame.unchanged {
                if let Some(old) = previous.get_mut(&index) {
                    old.region = frame.region;
                    frames.push(old.clone());
                }
            } else {
                previous.insert(index, frame.clone());
                frames.push(frame);
            }
        }
        place(app, settings, &frames)?;
        let original = corrected(
            &frames
                .iter()
                .map(|f| f.text.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            &settings.corrections, settings.whole_words,
        );
        if original.chars().count()>6000 {return Err("Hay demasiado texto entre las regiones. Reducí el área de lectura.".into());}
        let text_key = normalized(&original);
        let key=if !text_key.is_empty() && settings.mode=="over" {
            format!("{}\0{}",text_key,serde_json::to_string(&frames.iter().map(|f|(&f.region, &f.blocks)).collect::<Vec<_>>()).unwrap_or_default())
        } else {text_key.clone()};
        if key.is_empty() {
            stability.reset();
            update(app, Some(generation), |s| {
                s.message = if clipboard { "No hay texto en el portapapeles." }
                    else { "OCR listo. No se detectó texto: seleccioná la frase completa y comprobá el idioma OCR." }.into();
            });
            if last_nonempty.elapsed().as_millis() >= u128::from(settings.retain_ms) {
                update(app, Some(generation), |s| {
                    s.original.clear();
                    s.translation.clear();
                    s.blocks.clear();
                });
                last.clear();
            }
            if once {
                return Ok(());
            }
        } else {
            last_nonempty = Instant::now();
            if key == last {
                stability.reset();
                update(app, Some(generation), |s| s.message = "Traducción lista. Sin cambios en el texto.".into());
            } else if stability.ready(&text_key, Instant::now(), settings.stabilize_ms, once) {
                update(app, Some(generation), |s| {
                    s.phase = "translating".into();
                    s.message = "Traduciendo texto nuevo…".into();
                });
                let mut blocks = vec![];
                let translation = if settings.mode == "over" {
                    for frame in &frames {
                        for mut block in if settings.merge_paragraphs && !settings.vertical_text {merge_lines(frame.blocks.clone())} else {frame.blocks.clone()} {
                            block.text = corrected(&block.text, &settings.corrections, settings.whole_words);
                            block.x += f64::from(frame.region.x);
                            block.y += f64::from(frame.region.y);
                            blocks.push(block);
                        }
                    }
                    translate_blocks(app,generation,&client,&mut cache,&mut papago,&mut blocks,settings,&source).await?;
                    blocks
                        .iter()
                        .map(|b| b.translation.as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    translated(
                        app, generation, &client, &mut cache, &mut papago, &original, settings, &source,
                    )
                    .await?
                };
                let cache_error=cache.save(settings.persistent_cache).err();
                last = key.clone();
                stability.reset();
                update(app, Some(generation), |s| {
                    s.original = original.clone();
                    s.translation = translation.clone();
                    s.blocks = blocks;
                    s.message = cache_error.as_ref().map(|e|format!("Traducción lista. No se pudo guardar la caché: {e}")).unwrap_or_else(||"Traducción lista.".into());
                });
            } else {
                update(app, Some(generation), |s| s.message = "Texto detectado. Completando la lectura…".into());
            }
            if once {
                return Ok(());
            }
        }
        update(app, Some(generation), |s| s.phase = "waiting".into());
        let delay = if key != last && !key.is_empty() {
            settings.interval_ms.min(settings.stabilize_ms.max(100))
        } else {
            settings.interval_ms
        };
        // MORT schedules from the beginning of the OCR cycle, not after network/OCR work.
        tokio::time::sleep(Duration::from_millis(delay).saturating_sub(cycle_started.elapsed())).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_reuses_nonconsecutive_dialogues_and_stays_bounded() {
        let mut c = Cache {
            entries: VecDeque::new(),
        };
        c.put("en/es/A".into(), "uno".into());
        c.put("en/es/B".into(), "dos".into());
        assert_eq!(c.get("en/es/A"), Some("uno".into()));
        assert!(c.get("en/fr/A").is_none());
        for i in 0..300 {
            c.put(i.to_string(), "x".into());
        }
        assert_eq!(c.entries.len(), 256);
    }
    #[test]
    fn merge_preserves_separate_columns() {
        let line = |text: &str, x, y| Block {
            text: text.into(),
            x,
            y,
            width: 100.,
            height: 20.,
            translation: String::new(), background:None,foreground:None,
        };
        let blocks = merge_lines(vec![
            line("first", 0., 0.),
            line("second", 0., 23.),
            line("other", 200., 0.),
        ]);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].text, "first\nsecond");
        assert_eq!(blocks[0].height, 43.);
    }
}
