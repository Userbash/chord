#[path = "/home/sanya/orca/workspaces/project/feature-task-124-voice-devices-tauri-bff/rust/gui-spike/src/voice_devices.rs"]
mod voice_devices;

use voice_devices::{AudioDevice, AudioDeviceKind, VoiceDevicesState};

fn dev(id: &str, label: &str, kind: AudioDeviceKind, is_default: bool) -> AudioDevice {
    AudioDevice {
        id: id.to_owned(),
        label: label.to_owned(),
        kind,
        is_default,
    }
}

#[test]
fn debug_after_hotplug_fallback_leaks_device_labels() {
    let mut state = VoiceDevicesState::with_devices(vec![
        dev("mic-usb", "Система: USB Studio Microphone", AudioDeviceKind::Input, false),
        dev("mic-builtin", "Система: Built-in Microphone", AudioDeviceKind::Input, true),
        dev("spk-1", "Система: HDMI Output", AudioDeviceKind::Output, true),
    ]);
    state.select_input("mic-usb").unwrap();
    let events = state.update_device_list(vec![
        dev("mic-builtin", "Система: Built-in Microphone", AudioDeviceKind::Input, true),
        dev("spk-1", "Система: HDMI Output", AudioDeviceKind::Output, true),
    ]);
    assert_eq!(events.len(), 1);
    let dbg = format!("{state:?}");
    println!("--- state Debug after fallback ---\n{dbg}\n---");
    assert!(
        !dbg.contains("USB Studio Microphone") && !dbg.contains("Built-in Microphone"),
        "LEAK: fmt::Debug for VoiceDevicesState contains OS device labels"
    );
}
