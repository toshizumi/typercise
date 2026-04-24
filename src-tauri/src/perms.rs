#[cfg(target_os = "macos")]
mod mac {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
    use core_foundation::string::{CFString, CFStringRef};

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        static kAXTrustedCheckOptionPrompt: CFStringRef;
        fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> u8;
    }

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        // kIOHIDRequestTypeListenEvent = 1, kIOHIDRequestTypePostEvent = 0
        fn IOHIDRequestAccess(requestType: u32) -> u8;
        fn IOHIDCheckAccess(requestType: u32) -> u32; // 0=granted, 1=denied, 2=unknown
    }

    pub fn is_trusted(prompt: bool) -> bool {
        unsafe {
            let key: CFString = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
            let val = if prompt {
                CFBoolean::true_value()
            } else {
                CFBoolean::false_value()
            };
            let pairs: Vec<(CFString, CFBoolean)> = vec![(key, val)];
            let dict = CFDictionary::from_CFType_pairs(&pairs);
            AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef()) != 0
        }
    }

    pub fn input_monitoring_granted() -> bool {
        unsafe { IOHIDCheckAccess(1) == 0 }
    }

    pub fn request_input_monitoring() -> bool {
        unsafe { IOHIDRequestAccess(1) != 0 }
    }
}

#[cfg(target_os = "macos")]
pub fn check_accessibility() -> bool {
    mac::is_trusted(false)
}

#[cfg(target_os = "macos")]
pub fn request_accessibility() -> bool {
    mac::is_trusted(true)
}

#[cfg(target_os = "macos")]
pub fn check_input_monitoring() -> bool {
    mac::input_monitoring_granted()
}

#[cfg(target_os = "macos")]
pub fn request_input_monitoring() -> bool {
    mac::request_input_monitoring()
}

#[cfg(not(target_os = "macos"))]
pub fn check_accessibility() -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
pub fn request_accessibility() -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
pub fn check_input_monitoring() -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
pub fn request_input_monitoring() -> bool {
    true
}
