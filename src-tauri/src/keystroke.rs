#[cfg(target_os = "macos")]
use std::sync::Arc;

#[cfg(target_os = "macos")]
use crate::buffer::Buffer;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum KeyClass {
    Ignore,
    Count,
    Correction,
}

// 修飾キー単体のキーコード（macOS HID）
const MODIFIER_KEYCODES: &[u16] = &[
    54, // Right Cmd
    55, // Left Cmd
    56, // Shift
    57, // CapsLock
    58, // Option L
    59, // Control L
    60, // Shift R
    61, // Option R
    62, // Control R
    63, // Fn
];

// Backspace / Forward-Delete 系
const BACKSPACE: u16 = 51;
const FWD_DELETE: u16 = 117;

// Emacs/terminal 流: Ctrl+H = Backspace, Ctrl+D = Forward-Delete
const KEY_H: u16 = 4;
const KEY_D: u16 = 2;

/// 打鍵の分類。Ctrl フラグが立っているかは呼び出し側から渡す（macOS以外でもテスト可能）。
pub fn classify(keycode: u16, ctrl_pressed: bool) -> KeyClass {
    if MODIFIER_KEYCODES.contains(&keycode) {
        return KeyClass::Ignore;
    }
    if keycode == BACKSPACE || keycode == FWD_DELETE {
        return KeyClass::Correction;
    }
    if ctrl_pressed && (keycode == KEY_H || keycode == KEY_D) {
        return KeyClass::Correction;
    }
    KeyClass::Count
}

#[cfg(target_os = "macos")]
pub fn start(buf: Arc<Buffer>) {
    std::thread::spawn(move || {
        use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
        use core_graphics::event::{
            CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
            CGEventType, EventField,
        };

        let current = CFRunLoop::get_current();
        let buf_cb = Arc::clone(&buf);

        let tap = CGEventTap::new(
            CGEventTapLocation::HID,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::ListenOnly,
            vec![CGEventType::KeyDown],
            move |_proxy, event_type, event| {
                if matches!(event_type, CGEventType::KeyDown) {
                    let keycode =
                        event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
                    let flags = event.get_flags();
                    let ctrl = flags.contains(CGEventFlags::CGEventFlagControl);
                    match classify(keycode, ctrl) {
                        KeyClass::Ignore => {}
                        KeyClass::Count => buf_cb.inc_key(),
                        KeyClass::Correction => buf_cb.inc_correction(),
                    }
                }
                None
            },
        );

        match tap {
            Ok(tap) => unsafe {
                let src = tap
                    .mach_port
                    .create_runloop_source(0)
                    .expect("create_runloop_source");
                current.add_source(&src, kCFRunLoopCommonModes);
                tap.enable();
                tracing::info!("CGEventTap installed; listening for KeyDown events");
                CFRunLoop::run_current();
            },
            Err(_) => {
                tracing::error!(
                    "Failed to create CGEventTap. Accessibility permission is required."
                );
            }
        }
    });
}

#[cfg(not(target_os = "macos"))]
pub fn start(_buf: std::sync::Arc<crate::buffer::Buffer>) {
    // No-op on non-macOS platforms. This app is macOS-only.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifier_alone_is_ignored() {
        assert_eq!(classify(54, false), KeyClass::Ignore); // Cmd
        assert_eq!(classify(56, false), KeyClass::Ignore); // Shift
        assert_eq!(classify(63, false), KeyClass::Ignore); // Fn
    }

    #[test]
    fn backspace_is_correction() {
        assert_eq!(classify(51, false), KeyClass::Correction);
        assert_eq!(classify(51, true), KeyClass::Correction); // ⌃+Delete も訂正
    }

    #[test]
    fn forward_delete_is_correction() {
        assert_eq!(classify(117, false), KeyClass::Correction);
    }

    #[test]
    fn ctrl_h_and_ctrl_d_are_corrections() {
        assert_eq!(classify(4, true), KeyClass::Correction); // ⌃H
        assert_eq!(classify(2, true), KeyClass::Correction); // ⌃D
    }

    #[test]
    fn h_and_d_without_ctrl_are_count() {
        assert_eq!(classify(4, false), KeyClass::Count);
        assert_eq!(classify(2, false), KeyClass::Count);
    }

    #[test]
    fn other_letters_are_count() {
        assert_eq!(classify(0, false), KeyClass::Count); // A
        assert_eq!(classify(0, true), KeyClass::Count); // ⌃A (Emacs: line start) は訂正ではない
    }
}
