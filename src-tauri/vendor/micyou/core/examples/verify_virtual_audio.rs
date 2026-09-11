//! Hardware diagnostic: writes a synthetic tone to VB-CABLE only, reads CABLE Output.
//! Never captures a physical microphone or routes to speakers.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let outputs: Vec<_> = host
        .output_devices()?
        .filter_map(|d| d.name().ok())
        .filter(|n| n.to_lowercase().contains("vb-audio"))
        .collect();
    println!("Virtual render endpoints: {outputs:?}");
    let inputs: Vec<_> = host
        .input_devices()?
        .filter_map(|d| d.name().ok())
        .filter(|n| n.to_lowercase().contains("vb-audio"))
        .collect();
    println!("Virtual capture endpoints: {inputs:?}");
    let output_name = std::env::args()
        .nth(1)
        .or_else(|| {
            outputs
                .iter()
                .find(|n| n.to_lowercase().starts_with("cable input"))
                .cloned()
        })
        .ok_or("No CABLE Input endpoint")?;
    if !outputs.contains(&output_name) {
        return Err("Only enumerated VB-CABLE outputs allowed".into());
    }
    let input = host
        .input_devices()?
        .find(|d| {
            d.name()
                .unwrap_or_default()
                .to_lowercase()
                .starts_with("cable output")
        })
        .ok_or("No CABLE Output capture endpoint")?;
    let config = input.default_input_config()?;
    println!("Capture: {} {:?}", input.name()?, config);
    let measured = Arc::new(Mutex::new((0usize, 0.0f64)));
    let callback = measured.clone();
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => input.build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                let mut m = callback.lock().unwrap();
                m.0 += data.len();
                m.1 += data.iter().map(|s| (*s as f64).powi(2)).sum::<f64>();
            },
            |e| eprintln!("capture error: {e}"),
            None,
        )?,
        _ => return Err("Diagnostic requires float capture".into()),
    };
    stream.play()?;
    let mut output = micyou_audio::AudioOutputManager::new();
    output.start(Some(output_name.clone()), 300)?;
    for block in 0..150 {
        let tone: Vec<f32> = (0..480)
            .map(|i| {
                (2.0 * std::f32::consts::PI * 440.0 * (block * 480 + i) as f32 / 48000.0).sin()
                    * 0.1
            })
            .collect();
        output.push_audio_data(&tone, 1);
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(100));
    output.close();
    drop(stream);
    let (count, energy) = *measured.lock().unwrap();
    let rms = (energy / count.max(1) as f64).sqrt();
    println!("{output_name} -> CABLE Output: {count} samples, RMS={rms:.6}");
    if count == 0 || rms < 0.005 {
        return Err("No virtual microphone signal detected".into());
    }
    Ok(())
}
