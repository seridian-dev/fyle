//! Native macOS application menu integration.
#![allow(unexpected_cfgs)] // objc's sel_impl macro checks its own cargo-clippy cfg.

use cocoa::{
    appkit::{NSApp, NSApplication, NSMenu, NSMenuItem},
    base::{id, nil},
    foundation::NSString,
};
use objc::{
    declare::ClassDecl,
    msg_send,
    rc::StrongPtr,
    runtime::{Class, Object, Sel},
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    OnceLock,
};

static SETTINGS_REQUESTED: AtomicBool = AtomicBool::new(false);
static APP_CONTEXT: OnceLock<egui::Context> = OnceLock::new();

pub(crate) struct NativeMenu {
    _menu: StrongPtr,
    _target: StrongPtr,
}

impl NativeMenu {
    pub(crate) fn install(ctx: egui::Context) -> Self {
        let _ = APP_CONTEXT.set(ctx);
        unsafe {
            let target: id = msg_send![menu_target_class(), new];
            let target = StrongPtr::new(target);
            let root = StrongPtr::new(
                NSMenu::alloc(nil).initWithTitle_(NSString::alloc(nil).init_str("")),
            );
            let app_menu_item =
                StrongPtr::new(NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
                    NSString::alloc(nil).init_str("Fyle"),
                    selector("noAction:"),
                    NSString::alloc(nil).init_str(""),
                ));
            let app_menu = StrongPtr::new(
                NSMenu::alloc(nil).initWithTitle_(NSString::alloc(nil).init_str("Fyle")),
            );
            let app_menu_item_id: id = *app_menu_item;
            let app_menu_id: id = *app_menu;
            let root_id: id = *root;
            app_menu_item_id.setSubmenu_(app_menu_id);
            root_id.addItem_(app_menu_item_id);

            add_action_item(
                *app_menu,
                "About Fyle",
                selector("orderFrontStandardAboutPanel:"),
                nil,
                "",
            );
            app_menu_id.addItem_(NSMenuItem::separatorItem(nil));

            let settings_item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
                NSString::alloc(nil).init_str("Settings…"),
                selector("showPreferences:"),
                NSString::alloc(nil).init_str(","),
            );
            settings_item.setTarget_(*target);
            settings_item.setKeyEquivalentModifierMask_(
                cocoa::appkit::NSEventModifierFlags::NSCommandKeyMask,
            );
            app_menu_id.addItem_(settings_item);
            app_menu_id.addItem_(NSMenuItem::separatorItem(nil));

            add_action_item(app_menu_id, "Hide Fyle", selector("hide:"), nil, "h");
            add_action_item(
                app_menu_id,
                "Hide Others",
                selector("hideOtherApplications:"),
                nil,
                "",
            );
            add_action_item(
                app_menu_id,
                "Show All",
                selector("unhideAllApplications:"),
                nil,
                "",
            );
            app_menu_id.addItem_(NSMenuItem::separatorItem(nil));
            add_action_item(app_menu_id, "Quit Fyle", selector("terminate:"), nil, "q");

            NSApp().setMainMenu_(root_id);
            Self {
                _menu: root,
                _target: target,
            }
        }
    }

    pub(crate) fn take_settings_request(&self) -> bool {
        SETTINGS_REQUESTED.swap(false, Ordering::AcqRel)
    }
}

unsafe fn add_action_item(menu: id, title: &str, action: Sel, target: id, key: &str) {
    let item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
        NSString::alloc(nil).init_str(title),
        action,
        NSString::alloc(nil).init_str(key),
    );
    if !target.is_null() {
        item.setTarget_(target);
    }
    menu.addItem_(item);
}

extern "C" fn show_preferences(_: &Object, _: Sel, _: id) {
    SETTINGS_REQUESTED.store(true, Ordering::Release);
    if let Some(ctx) = APP_CONTEXT.get() {
        ctx.request_repaint();
    }
}

fn menu_target_class() -> &'static Class {
    static CLASS: OnceLock<&'static Class> = OnceLock::new();
    CLASS.get_or_init(|| unsafe {
        let superclass = Class::get("NSObject").expect("NSObject class missing");
        let mut declaration = ClassDecl::new("FyleMenuActionTarget", superclass)
            .expect("Fyle menu action class already exists");
        declaration.add_method(
            selector("showPreferences:"),
            show_preferences as extern "C" fn(&Object, Sel, id),
        );
        declaration.register()
    })
}

fn selector(name: &str) -> Sel {
    Sel::register(name)
}
