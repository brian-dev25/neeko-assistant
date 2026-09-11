//! Validates bundled AEC7 inference without capturing a microphone or speaker.
#![cfg(feature = "noise-suppression")]
#[test]
#[ignore = "requires the packaged ONNX Runtime DLL and AEC7 model"]
fn packaged_aec_model_processes_finite_audio() {
    use micyou_audio::{AudioDspSettings, DspProcessor};
    use std::sync::{Arc, RwLock};
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../addons/phone-microphone/resources");
    micyou_audio::init_ort_runtime(&root.join("onnxruntime.dll")).unwrap();
    let mut processor = DspProcessor::new(
        Arc::new(RwLock::new(AudioDspSettings {
            aec_enabled: true,
            ..Default::default()
        })),
        Some(root),
    );
    for _ in 0..100 {
        processor.set_far_end_audio(&vec![0.0; 480]);
        let mut near: Vec<f32> = (0..480).map(|i| (i as f32 * 0.1).sin() * 0.02).collect();
        processor.process(&mut near, 1, 0.0);
        assert!(near.iter().all(|s| s.is_finite()));
        assert!(
            processor.take_aec_failure().is_none(),
            "AEC runtime/model failed"
        );
    }
}
