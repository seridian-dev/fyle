//! macOS Quick Look popout used by the Space shortcut.
#![allow(unexpected_cfgs)] // objc's sel_impl macro checks its own cargo-clippy cfg.

use cocoa::base::{id, nil};
use objc::{
    declare::ClassDecl,
    msg_send,
    runtime::{Class, Object, Protocol, Sel},
};
use std::{
    ffi::CString,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock,
    },
};

static PREVIEW_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);
static DATA_SOURCE: OnceLock<usize> = OnceLock::new();
static FRAMEWORK_HANDLE: OnceLock<usize> = OnceLock::new();
static APP_READY: AtomicBool = AtomicBool::new(false);

#[link(name = "QuickLookUI", kind = "framework")]
unsafe extern "C" {}

pub(crate) fn install() {
    let handle = *FRAMEWORK_HANDLE.get_or_init(|| unsafe {
        let path = CString::new("/System/Library/Frameworks/QuickLookUI.framework/QuickLookUI")
            .expect("Quick Look framework path has no NUL bytes");
        libc::dlopen(path.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL) as usize
    });
    if handle == 0 {
        let error = unsafe { libc::dlerror() };
        let error = if error.is_null() {
            "unknown dynamic loader error".to_owned()
        } else {
            unsafe { std::ffi::CStr::from_ptr(error) }
                .to_string_lossy()
                .into_owned()
        };
        log::error!("Could not load the macOS Quick Look framework: {error}");
        return;
    }
    log::info!("Loaded the macOS Quick Look framework");
    APP_READY.store(true, Ordering::Release);
}

/// Toggle the native Quick Look panel for a local file.
pub(crate) fn toggle(path: Option<&Path>) -> bool {
    if !APP_READY.load(Ordering::Acquire) {
        log::warn!("Quick Look was requested before its framework was ready");
        return false;
    }
    log::info!(
        "Quick Look toggle requested for {}",
        path.map_or_else(|| "no selection".to_owned(), |p| p.display().to_string())
    );
    unsafe {
        let panel = shared_panel();
        if panel.is_null() {
            log::error!("The macOS Quick Look panel class is unavailable");
            return false;
        }
        let visible: bool = msg_send![panel, isVisible];
        if visible {
            let _: () = msg_send![panel, close];
            true
        } else if let Some(path) = path {
            set_path(path);
            show_panel(panel);
            let visible: bool = msg_send![panel, isVisible];
            visible
        } else {
            false
        }
    }
}

/// Keep Quick Look synchronized with the selected file while its panel is open.
pub(crate) fn selection_changed(path: Option<&Path>) {
    if !APP_READY.load(Ordering::Acquire) {
        return;
    }
    unsafe {
        let panel = shared_panel();
        if panel.is_null() {
            return;
        }
        let visible: bool = msg_send![panel, isVisible];
        if !visible {
            return;
        }
        if let Some(path) = path {
            set_path(path);
            let _: () = msg_send![panel, reloadData];
        } else {
            let _: () = msg_send![panel, close];
        }
    }
}

fn set_path(path: &Path) {
    if let Ok(mut slot) = PREVIEW_PATH.lock() {
        *slot = Some(path.to_path_buf());
    }
}

unsafe fn shared_panel() -> id {
    let Some(class) = Class::get("QLPreviewPanel") else {
        return nil;
    };
    let panel: id = msg_send![class, sharedPreviewPanel];
    panel
}

unsafe fn show_panel(panel: id) {
    let source = *DATA_SOURCE.get_or_init(|| {
        let object: id = msg_send![preview_source_class(), new];
        object as usize
    }) as id;
    let _: () = msg_send![panel, setDataSource: source];
    let _: () = msg_send![panel, reloadData];
    let _: () = msg_send![panel, makeKeyAndOrderFront: nil];
    log::info!("Opened the macOS Quick Look panel");
}

extern "C" fn item_count(_: &Object, _: Sel, _: id) -> isize {
    PREVIEW_PATH
        .lock()
        .map(|path| isize::from(path.is_some()))
        .unwrap_or(0)
}

extern "C" fn preview_item(_: &Object, _: Sel, _: id, index: isize) -> id {
    if index != 0 {
        return nil;
    }
    let Ok(path) = PREVIEW_PATH.lock() else {
        return nil;
    };
    let Some(path) = path.as_ref() else {
        return nil;
    };
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return nil;
    };
    unsafe {
        let class = Class::get("NSURL").expect("NSURL class missing");
        let url: id = msg_send![class,
            fileURLWithFileSystemRepresentation: path.as_ptr()
            isDirectory: false
            relativeToURL: nil
        ];
        url
    }
}

fn preview_source_class() -> &'static Class {
    static CLASS: OnceLock<&'static Class> = OnceLock::new();
    CLASS.get_or_init(|| unsafe {
        let superclass = Class::get("NSObject").expect("NSObject class missing");
        let mut declaration = ClassDecl::new("FyleQuickLookDataSource", superclass)
            .expect("Quick Look data source class already exists");
        if let Some(protocol) = Protocol::get("QLPreviewPanelDataSource") {
            declaration.add_protocol(protocol);
        }
        declaration.add_method(
            selector("numberOfPreviewItemsInPreviewPanel:"),
            item_count as extern "C" fn(&Object, Sel, id) -> isize,
        );
        declaration.add_method(
            selector("previewPanel:previewItemAtIndex:"),
            preview_item as extern "C" fn(&Object, Sel, id, isize) -> id,
        );
        declaration.register()
    })
}

fn selector(name: &str) -> Sel {
    Sel::register(name)
}
